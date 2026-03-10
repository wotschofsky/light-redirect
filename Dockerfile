# Stage 1: Builder
FROM rust:alpine AS builder

ARG TARGETARCH

RUN apk add --no-cache musl-dev

RUN rustup target add x86_64-unknown-linux-musl && \
    rustup target add aarch64-unknown-linux-musl

ENV RUSTFLAGS="-C target-feature=+crt-static"

WORKDIR /app

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo 'fn main() {}' > src/main.rs && \
    case "$TARGETARCH" in \
        amd64) cargo build --release --target x86_64-unknown-linux-musl ;; \
        arm64) cargo build --release --target aarch64-unknown-linux-musl ;; \
        *)     cargo build --release --target x86_64-unknown-linux-musl ;; \
    esac && \
    rm src/main.rs

# Build the real binary
COPY src ./src
RUN touch src/main.rs && \
    case "$TARGETARCH" in \
        amd64) cargo build --release --target x86_64-unknown-linux-musl && \
               strip target/x86_64-unknown-linux-musl/release/light-redirect && \
               cp target/x86_64-unknown-linux-musl/release/light-redirect /light-redirect ;; \
        arm64) cargo build --release --target aarch64-unknown-linux-musl && \
               strip target/aarch64-unknown-linux-musl/release/light-redirect && \
               cp target/aarch64-unknown-linux-musl/release/light-redirect /light-redirect ;; \
        *)     cargo build --release --target x86_64-unknown-linux-musl && \
               strip target/x86_64-unknown-linux-musl/release/light-redirect && \
               cp target/x86_64-unknown-linux-musl/release/light-redirect /light-redirect ;; \
    esac

# Stage 2: Runtime
FROM scratch

COPY --from=builder /light-redirect /light-redirect

EXPOSE 80

ENTRYPOINT ["/light-redirect"]
