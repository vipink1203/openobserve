# syntax=docker/dockerfile:1
FROM public.ecr.aws/docker/library/node:18.20.4-bookworm AS webBuilder
WORKDIR /web
COPY ./web /web/

RUN npm install
RUN NODE_OPTIONS="--max-old-space-size=8192" npm run build

FROM public.ecr.aws/zinclabs/rust:bookworm-sccache AS builder

# RUN rustup toolchain install nightly-2025-11-11
# RUN rustup default nightly-2025-11-11
# RUN rustup target add x86_64-unknown-linux-gnu
# RUN diff -u <(rustc --print cfg) <(rustc -C target-cpu=native --print cfg)
RUN rustc --version && sccache --version

# Install dependencies for SAML (xmlsec feature)
RUN apt-get update && apt-get install -y --no-install-recommends \
    libclang-dev \
    clang \
    pkg-config \
    libxml2-dev \
    libxmlsec1-dev \
    libxmlsec1-openssl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /openobserve
COPY . /openobserve
COPY --from=webBuilder /web/dist web/dist

# Reduce memory usage during compilation
ENV CARGO_BUILD_JOBS=1
ENV CARGO_NET_RETRY=10
RUN cargo build --release

# FROM gcr.io/distroless/cc as runtime
FROM public.ecr.aws/debian/debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    htop \
    iftop \
    sysstat \
    procps \
    lsof \
    net-tools \
    libxml2 \
    libxmlsec1 \
    libxmlsec1-openssl \
    && rm -rf /var/lib/apt/lists/*
RUN update-ca-certificates
COPY --from=builder /openobserve/target/release/openobserve /
RUN ["/openobserve", "init-dir", "-p", "/data/"]
CMD ["/openobserve"]
