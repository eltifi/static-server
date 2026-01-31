# Builder Stage
FROM rust:alpine AS builder

WORKDIR /app

# Install build dependencies for Alpine (musl-dev required)
RUN apk add --no-cache musl-dev

# Copy manifest
COPY Cargo.toml Cargo.lock ./

# Create dummy main.rs to build dependencies
RUN echo "fn main() {}" > main.rs

# Build release dependencies (this layer is cached)
RUN cargo build --release

# Clean up dummy build artifacts to force rebuild of main.rs
RUN rm main.rs target/release/deps/static_server*

# Copy actual source
COPY main.rs ./

# Build release binary
RUN cargo build --release

# Runtime Stage
FROM alpine:latest

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/static-server /app/static-server

# Build arguments
ARG PORT=80

# Environment Defaults
ENV PORT=${PORT}
ENV WEB_ROOT=/var/www

# Expose port
EXPOSE ${PORT}

# Mount point for static files
VOLUME ["/var/www"]

# Run
CMD ["/app/static-server"]
