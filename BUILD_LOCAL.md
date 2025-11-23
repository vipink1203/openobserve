# Local Build Instructions for macOS

## Prerequisites

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Install dependencies
brew install protobuf pkg-config libxml2 libxmlsec1

# Set environment variable for protobuf
export PROTOC_INCLUDE=/opt/homebrew/include
```

## Build Steps

```bash
# 1. Build the frontend
cd web
npm install
npm run build
cd ..

# 2. Build the backend (this will take 10-15 minutes)
cargo build --release

# The binary will be at: target/release/openobserve
```

## Run Locally

```bash
# Run OpenObserve with SAML enabled
ZO_ROOT_USER_EMAIL="root@example.com" \
ZO_ROOT_USER_PASSWORD="Complexpass#123" \
ZO_DATA_DIR="./data" \
./target/release/openobserve
```

Then access at http://localhost:5080

## Run with Docker Compose (using local binary)

If you still want to use the test SAML IdP:

```bash
# Start just the SAML IdP
docker-compose -f docker-compose-saml.yml up saml-idp -d

# Run OpenObserve locally (in another terminal)
ZO_ROOT_USER_EMAIL="root@example.com" \
ZO_ROOT_USER_PASSWORD="Complexpass#123" \
./target/release/openobserve
```

Access:
- OpenObserve: http://localhost:5080
- Test SAML IdP: http://localhost:8080
- Test user: user1@example.com / user1pass
