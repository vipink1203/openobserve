// Copyright 2025 OpenObserve Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use actix_web::{HttpRequest, HttpResponse, cookie, get, http, post, web};
use config::{get_config, meta::user::UserRole, utils::base64};
use samael::{
    metadata::EntityDescriptor,
    service_provider::{ServiceProvider, ServiceProviderBuilder},
    traits::ToXml,
};
use serde::{Deserialize, Serialize};

use crate::{
    common::meta::user::AuthTokens,
    service::users,
};

const SAML_SESSION_DURATION: i64 = 43200; // 12 hours

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SAMLResponse {
    #[serde(rename = "SAMLResponse")]
    pub saml_response: String,
    #[serde(rename = "RelayState")]
    pub relay_state: Option<String>,
}

/// Get SAML service provider instance from database config
async fn get_service_provider() -> Result<ServiceProvider, Box<dyn std::error::Error>> {
    // Get SAML config from database (or fallback to env vars)
    let saml_config = crate::service::db::saml::get().await?;

    if !saml_config.enabled {
        return Err("SAML is not enabled".into());
    }

    if saml_config.sp_entity_id.is_empty() || saml_config.acs_url.is_empty() {
        return Err("SAML SP Entity ID and ACS URL must be configured".into());
    }

    if saml_config.idp_metadata_xml.is_empty() {
        return Err("SAML IdP metadata must be configured".into());
    }

    let idp_metadata: EntityDescriptor = saml_config.idp_metadata_xml.parse()?;

    let sp = ServiceProviderBuilder::default()
        .entity_id(saml_config.sp_entity_id)
        .acs_url(saml_config.acs_url)
        .idp_metadata(idp_metadata)
        .allow_idp_initiated(saml_config.allow_idp_initiated)
        .build()?;

    Ok(sp)
}

/// SAML Login - Initiate SP-initiated login
#[utoipa::path(
    context_path = "/auth/saml",
    tag = "Auth",
    operation_id = "SAMLLogin",
    summary = "Initiate SAML SSO login",
    description = "Initiates a SAML authentication flow by redirecting to the configured IdP",
    responses(
        (status = 302, description = "Redirect to IdP"),
        (status = 500, description = "SAML not configured"),
    )
)]
#[get("/login")]
pub async fn saml_login(_req: HttpRequest) -> Result<HttpResponse, actix_web::Error> {
    let sp = match get_service_provider().await {
        Ok(sp) => sp,
        Err(e) => {
            log::error!("Failed to initialize SAML SP: {}", e);
            return Ok(HttpResponse::InternalServerError()
                .json(format!("SAML configuration error: {}", e)));
        }
    };

    // Get IdP SSO URL from metadata
    let idp_sso_url = sp
        .idp_metadata
        .idp_sso_descriptors
        .first()
        .and_then(|desc| desc.single_sign_on_services.first())
        .map(|sso| sso.location.clone())
        .ok_or_else(|| "IdP SSO URL not found in metadata")?;

    // Generate authentication request
    match sp.make_authentication_request(&idp_sso_url) {
        Ok(authn_request) => {
            // Serialize and encode the authentication request
            let authn_xml = authn_request.to_xml().map_err(|e| {
                log::error!("Failed to serialize AuthnRequest: {}", e);
                format!("Failed to create authentication request: {}", e)
            })?;

            let encoded_request = base64::encode(&authn_xml);

            // Build redirect URL with SAMLRequest parameter
            let redirect_url = format!("{}?SAMLRequest={}", idp_sso_url,
                urlencoding::encode(&encoded_request));

            Ok(HttpResponse::Found()
                .append_header((http::header::LOCATION, redirect_url))
                .finish())
        }
        Err(e) => {
            log::error!("Failed to create authentication request: {}", e);
            Ok(HttpResponse::InternalServerError()
                .json(format!("Failed to create authentication request: {}", e)))
        }
    }
}

/// SAML ACS - Handle SAML response from IdP
#[utoipa::path(
    context_path = "/auth/saml",
    tag = "Auth",
    operation_id = "SAMLCallback",
    summary = "Handle SAML response",
    description = "Processes SAML assertion from IdP and creates user session",
    request_body(
        content = inline(SAMLResponse),
        description = "SAML response from IdP",
        content_type = "application/x-www-form-urlencoded"
    ),
    responses(
        (status = 302, description = "Redirect to application"),
        (status = 401, description = "Authentication failed"),
    )
)]
#[post("/acs")]
pub async fn saml_acs(
    form: web::Form<SAMLResponse>,
    _req: HttpRequest,
) -> Result<HttpResponse, actix_web::Error> {
    let sp = match get_service_provider().await {
        Ok(sp) => sp,
        Err(e) => {
            log::error!("Failed to initialize SAML SP: {}", e);
            return Ok(HttpResponse::InternalServerError()
                .json(format!("SAML configuration error: {}", e)));
        }
    };

    // Decode SAML response (base64 decode returns String)
    let saml_response_decoded = match base64::decode(&form.saml_response) {
        Ok(decoded) => decoded,
        Err(e) => {
            log::error!("Failed to decode SAML response: {}", e);
            return Ok(HttpResponse::BadRequest().json("Invalid SAML response encoding"));
        }
    };

    // Parse SAML response
    let assertion = match sp.parse_xml_response(&saml_response_decoded, None) {
        Ok(assertion) => assertion,
        Err(e) => {
            log::error!("Failed to parse SAML response: {}", e);
            return Ok(HttpResponse::Unauthorized().json("SAML authentication failed"));
        }
    };

    // Get SAML config to extract user info
    let saml_config = match crate::service::db::saml::get().await {
        Ok(config) => config,
        Err(e) => {
            log::error!("Failed to get SAML config: {}", e);
            return Ok(HttpResponse::InternalServerError()
                .json("Failed to retrieve SAML configuration"));
        }
    };

    // Extract email from SAML assertion
    // First try to get from NameID in subject
    let email = assertion
        .subject
        .as_ref()
        .and_then(|subj| subj.name_id.as_ref())
        .map(|name_id| name_id.value.as_str())
        .unwrap_or_else(|| {
            // Fallback: try to extract from attributes
            assertion
                .attribute_statements
                .as_ref()
                .and_then(|statements| statements.first())
                .and_then(|statement| {
                    statement.attributes.iter().find(|attr| {
                        attr.name.as_deref() == Some(&saml_config.email_attribute)
                    })
                })
                .and_then(|attr| attr.values.first())
                .and_then(|val| val.value.as_deref())
                .unwrap_or("")
        })
        .to_lowercase();

    // Extract name from attributes
    let name = assertion
        .attribute_statements
        .as_ref()
        .and_then(|statements| statements.first())
        .and_then(|statement| {
            statement.attributes.iter().find(|attr| {
                attr.name.as_deref() == Some(&saml_config.name_attribute)
            })
        })
        .and_then(|attr| attr.values.first())
        .and_then(|val| val.value.as_deref())
        .unwrap_or(&email)
        .to_string();

    if email.is_empty() {
        log::error!("No email found in SAML assertion");
        return Ok(HttpResponse::Unauthorized()
            .json("Email attribute not found in SAML response"));
    }

    log::info!("SAML authentication successful for user: {}", email);

    // Create or update user in database
    let user_role = match saml_config.default_role.as_str() {
        "admin" => UserRole::Admin,
        "editor" => UserRole::Editor,
        "viewer" => UserRole::Viewer,
        _ => UserRole::Admin,
    };

    // Ensure user exists in the system (get_user returns Option)
    if users::get_user(None, &email).await.is_none() {
        log::info!("Creating new user from SAML: {}", email);
        // User doesn't exist, create them
        let user_request = crate::common::meta::user::UserRequest {
            email: email.clone(),
            first_name: name.clone(),
            last_name: String::new(),
            password: String::new(), // No password for SAML users
            role: crate::common::meta::user::UserOrgRole {
                base_role: user_role,
                custom_role: None,
            },
            is_external: true,
            token: None,
        };

        if let Err(e) = users::post_user(&saml_config.default_org, user_request, &email).await {
            log::error!("Failed to create SAML user: {}", e);
            return Ok(
                HttpResponse::InternalServerError().json("Failed to create user session")
            );
        }
    }

    // Create session tokens
    let session_token = format!(
        "saml_session_{}",
        config::utils::rand::generate_random_string(32)
    );

    // Store session (session_token is stored with email as key for validation)
    if let Err(e) = crate::service::db::session::set(&email, &session_token).await {
        log::error!("Failed to create session: {}", e);
        return Ok(HttpResponse::InternalServerError().json("Failed to create user session"));
    }
    log::info!("Session created for SAML user: {}", email);

    // Create auth cookie
    let tokens = AuthTokens {
        access_token: format!("Bearer {}", session_token),
        refresh_token: String::new(),
    };

    let cfg = get_config();
    let tokens_json = config::utils::json::to_string(&tokens).unwrap();
    let tokens_encoded = base64::encode(&tokens_json);

    let mut auth_cookie = cookie::Cookie::new("auth_tokens", tokens_encoded);
    auth_cookie.set_expires(
        cookie::time::OffsetDateTime::now_utc()
            + cookie::time::Duration::seconds(SAML_SESSION_DURATION),
    );
    auth_cookie.set_http_only(true);
    auth_cookie.set_secure(cfg.auth.cookie_secure_only);
    auth_cookie.set_path("/");

    if cfg.auth.cookie_same_site_lax {
        auth_cookie.set_same_site(cookie::SameSite::Lax);
    } else {
        auth_cookie.set_same_site(cookie::SameSite::None);
    }

    // Create ID token for frontend
    let id_token = config::utils::json::json!({
        "email": email,
        "name": name,
    });

    let url = format!(
        "{}{}/web/cb#id_token={}.{}",
        cfg.common.web_url,
        cfg.common.base_uri,
        "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9", // JWT header placeholder
        base64::encode(&id_token.to_string())
    );

    Ok(HttpResponse::Found()
        .append_header((http::header::LOCATION, url))
        .cookie(auth_cookie)
        .finish())
}

/// SAML Metadata - Provide SP metadata for IdP configuration
#[utoipa::path(
    context_path = "/auth/saml",
    tag = "Auth",
    operation_id = "SAMLMetadata",
    summary = "Get SAML SP metadata",
    description = "Returns SAML Service Provider metadata XML for IdP configuration",
    responses(
        (status = 200, description = "SP metadata XML", content_type = "application/xml"),
        (status = 500, description = "SAML not configured"),
    )
)]
#[get("/metadata")]
pub async fn saml_metadata(_req: HttpRequest) -> Result<HttpResponse, actix_web::Error> {
    let sp = match get_service_provider().await {
        Ok(sp) => sp,
        Err(e) => {
            log::error!("Failed to initialize SAML SP: {}", e);
            return Ok(HttpResponse::InternalServerError()
                .json(format!("SAML configuration error: {}", e)));
        }
    };

    match sp.metadata() {
        Ok(metadata) => {
            // Use ToXml trait to convert to XML string
            let xml = metadata.to_xml().map_err(|e| {
                log::error!("Failed to serialize metadata: {}", e);
                format!("Failed to generate metadata: {}", e)
            })?;

            Ok(HttpResponse::Ok()
                .content_type("application/xml")
                .body(xml))
        }
        Err(e) => {
            log::error!("Failed to generate metadata: {}", e);
            Ok(HttpResponse::InternalServerError()
                .json(format!("Failed to generate metadata: {}", e)))
        }
    }
}

/// SAML Logout - Handle logout request
#[utoipa::path(
    context_path = "/auth/saml",
    tag = "Auth",
    operation_id = "SAMLLogout",
    summary = "SAML logout",
    description = "Handles SAML logout and clears session",
    responses(
        (status = 302, description = "Redirect to application"),
    )
)]
#[get("/logout")]
pub async fn saml_logout(_req: HttpRequest) -> Result<HttpResponse, actix_web::Error> {
    let cfg = get_config();

    // Clear auth cookie
    let mut auth_cookie = cookie::Cookie::new("auth_tokens", "");
    auth_cookie.set_expires(cookie::time::OffsetDateTime::now_utc());
    auth_cookie.set_http_only(true);
    auth_cookie.set_secure(cfg.auth.cookie_secure_only);
    auth_cookie.set_path("/");

    if cfg.auth.cookie_same_site_lax {
        auth_cookie.set_same_site(cookie::SameSite::Lax);
    } else {
        auth_cookie.set_same_site(cookie::SameSite::None);
    }

    let redirect_url = format!("{}{}/", cfg.common.web_url, cfg.common.base_uri);

    Ok(HttpResponse::Found()
        .append_header((http::header::LOCATION, redirect_url))
        .cookie(auth_cookie)
        .finish())
}
