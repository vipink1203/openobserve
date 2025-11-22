# Local SAML Testing with Docker

This guide provides simple steps to launch OpenObserve locally in a Docker container and test the SAML authentication feature.

## Prerequisites

- Docker installed on your machine
- Git (to clone the repository)

## Quick Start

### Option 1: Test with Mock SAML IdP (Recommended for Testing)

This uses a pre-configured test SAML identity provider.

1. **Clone the repository:**
   ```bash
   git clone https://github.com/vipink1203/openobserve
   cd openobserve
   ```

2. **Start the test environment:**
   ```bash
   docker-compose -f docker-compose-saml.yml up -d
   ```

3. **Access OpenObserve:**
   - Open your browser: `http://localhost:5080`
   - You should see the login page with SAML option

4. **Test SAML login:**
   - Click "Login with AWS SSO (SAML)"
   - Use test credentials:
     - Email: `user1@example.com` / Password: `user1pass`
     - Email: `admin@example.com` / Password: `adminpass`

5. **View logs (if needed):**
   ```bash
   docker-compose -f docker-compose-saml.yml logs -f openobserve
   ```

6. **Stop the environment:**
   ```bash
   docker-compose -f docker-compose-saml.yml down
   ```

### Option 2: Simple Docker Run (Without SAML)

To launch OpenObserve without SAML for basic testing:

1. **Run the container:**
   ```bash
   docker run -d \
     --name openobserve \
     -v $PWD/data:/data \
     -p 5080:5080 \
     -e ZO_ROOT_USER_EMAIL="root@example.com" \
     -e ZO_ROOT_USER_PASSWORD="Complexpass#123" \
     public.ecr.aws/zinclabs/openobserve:latest
   ```

2. **Access OpenObserve:**
   - Open your browser: `http://localhost:5080`
   - Login with:
     - Email: `root@example.com`
     - Password: `Complexpass#123`

3. **Configure SAML via UI:**
   - Go to Settings → SAML Configuration
   - Fill in your SAML IdP details
   - Save and test

4. **View logs:**
   ```bash
   docker logs -f openobserve
   ```

5. **Stop the container:**
   ```bash
   docker stop openobserve
   docker rm openobserve
   ```

### Option 3: Docker Run with SAML Environment Variables

To test SAML with environment variables:

1. **Create a `.env` file:**
   ```bash
   cat > .env << 'EOF'
   ZO_ROOT_USER_EMAIL=admin@example.com
   ZO_ROOT_USER_PASSWORD=Complexpass#123
   ZO_SAML_ENABLED=true
   ZO_SAML_SP_ENTITY_ID=http://localhost:5080/auth/saml/metadata
   ZO_SAML_ACS_URL=http://localhost:5080/auth/saml/acs
   ZO_SAML_IDP_METADATA_XML=<paste your IdP metadata XML here>
   ZO_SAML_DEFAULT_ORG=default
   ZO_SAML_DEFAULT_ROLE=admin
   EOF
   ```

2. **Run with environment file:**
   ```bash
   docker run -d \
     --name openobserve \
     --env-file .env \
     -v $PWD/data:/data \
     -p 5080:5080 \
     public.ecr.aws/zinclabs/openobserve:latest
   ```

3. **Access and test:**
   - Open: `http://localhost:5080`
   - SAML login button should be visible

## Building and Running from Source

If you want to test with your local code changes:

1. **Build the Docker image:**
   ```bash
   docker build -t openobserve:local -f deploy/build/Dockerfile .
   ```

2. **Run your local build:**
   ```bash
   docker run -d \
     --name openobserve \
     -v $PWD/data:/data \
     -p 5080:5080 \
     -e ZO_ROOT_USER_EMAIL="root@example.com" \
     -e ZO_ROOT_USER_PASSWORD="Complexpass#123" \
     openobserve:local
   ```

## Testing SAML with AWS SSO

If you have AWS SSO configured:

1. **Get your AWS SSO metadata:**
   - Go to AWS IAM Identity Center
   - Navigate to your OpenObserve SAML application
   - Download the metadata XML

2. **Launch OpenObserve:**
   ```bash
   docker run -d \
     --name openobserve \
     -v $PWD/data:/data \
     -p 5080:5080 \
     -e ZO_ROOT_USER_EMAIL="admin@example.com" \
     -e ZO_ROOT_USER_PASSWORD="Complexpass#123" \
     public.ecr.aws/zinclabs/openobserve:latest
   ```

3. **Configure SAML in UI:**
   - Login as admin: `http://localhost:5080`
   - Go to Settings → SAML Configuration
   - Enable SAML
   - Set SP Entity ID: `http://localhost:5080/auth/saml/metadata`
   - Set ACS URL: `http://localhost:5080/auth/saml/acs`
   - Paste your AWS SSO metadata XML
   - Save configuration

4. **Configure AWS SSO:**
   - In AWS SSO, set:
     - ACS URL: `http://localhost:5080/auth/saml/acs`
     - Audience: `http://localhost:5080/auth/saml/metadata`
   - Map attributes:
     - email → ${user:email}
     - name → ${user:name}

5. **Test login:**
   - Open new browser window: `http://localhost:5080`
   - Click "Login with AWS SSO (SAML)"
   - Authenticate with AWS SSO

## Troubleshooting

**SAML button not showing:**
- Check `http://localhost:5080/config` - verify `saml_enabled: true`
- Restart container: `docker restart openobserve`

**Authentication fails:**
- Check logs: `docker logs openobserve | grep -i saml`
- Verify metadata XML is correct
- Ensure ACS URL and Entity ID match in both systems

**Cannot access on localhost:**
- Verify port 5080 is not in use: `lsof -i :5080` (Mac/Linux) or `netstat -ano | findstr :5080` (Windows)
- Check container is running: `docker ps | grep openobserve`

**Data persistence:**
- Data is stored in `./data` directory
- To start fresh: `rm -rf ./data` before running container

## Useful Commands

```bash
# Check if container is running
docker ps | grep openobserve

# View real-time logs
docker logs -f openobserve

# Enter container shell
docker exec -it openobserve sh

# Check configuration
curl http://localhost:5080/config

# Get SAML metadata
curl http://localhost:5080/auth/saml/metadata

# Restart container
docker restart openobserve

# Remove all data and start fresh
docker stop openobserve
docker rm openobserve
rm -rf ./data
```

## Next Steps

- For production deployment, see `CONTRIBUTING.md`
- For detailed SAML setup with AWS SSO, see `docs/SAML_AWS_SSO_SETUP.md`
- For implementation details, see `docs/SAML_IMPLEMENTATION_SUMMARY.md`
