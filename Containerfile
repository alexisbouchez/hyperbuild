# Containerfile for Hyperbuild - Rust Container Builder
# This Containerfile demonstrates how to build containers with hyperbuild
# and push them to hyperregistry

FROM rust:1.80-alpine AS builder

# Install build dependencies
RUN apk add --no-cache musl-dev openssl-dev pkg-config

# Create app directory
WORKDIR /app

# Copy manifest and lock files
COPY rust-container-builder/Cargo.toml rust-container-builder/Cargo.lock ./

# Create a dummy main.rs to allow cargo to download dependencies
RUN mkdir src && echo 'fn main() {}' > src/main.rs

# Download and compile dependencies
RUN cargo build --release

# Clean up the dummy source
RUN rm -rf src

# Copy the actual source code
COPY rust-container-builder/src ./src

# Build the application
RUN cargo build --release

# Production stage
FROM alpine:latest

# Install runtime dependencies
RUN apk add --no-cache ca-certificates curl tar gzip

# Create non-root user
RUN addgroup -g 1001 -S appuser && \
    adduser -u 1001 -S appuser -G appuser

# Copy the binary from builder stage
COPY --from=builder /app/target/release/rust-container-builder /usr/local/bin/hyperbuild

# Create necessary directories
RUN mkdir -p /workspace && chown -R appuser:appuser /workspace

# Switch to non-root user
USER appuser

# Set working directory
WORKDIR /workspace

# Expose port if needed for any API functionality
EXPOSE 8080

# Default command
CMD ["hyperbuild", "--help"]

# Example usage:
# Build an image: hyperbuild -c /path/to/context -f /path/to/Dockerfile -i my-image
# Push to registry: (implementation would need to be added to hyperbuild)