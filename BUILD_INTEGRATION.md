# Hyperbuild + Hyperregistry Integration Guide

This guide explains how to use hyperbuild (the Rust container builder) to build container images and push them to hyperregistry (the Rust container registry).

## Prerequisites

- Docker installed
- Rust toolchain installed (for building hyperbuild)
- PostgreSQL and Redis (for hyperregistry)

## Building Hyperbuild

### Using the Containerfile

```bash
# Build the hyperbuild container
cd /path/to/hyperbuild
docker build -t hyperbuild:latest -f Containerfile .
```

### Running Hyperbuild

```bash
# Run hyperbuild to build a container image
docker run --rm -v $(pwd)/my-app:/workspace/my-app hyperbuild:latest \
    hyperbuild -c /workspace/my-app -f /workspace/my-app/Dockerfile -i my-app:latest
```

## Setting up Hyperregistry

### Building Hyperregistry

```bash
# Navigate to the hyperregistry directory
cd /path/to/hyperregistry

# Build the registry
cd rust-registry
cargo build --release
```

### Running Hyperregistry

```bash
# Set environment variables
export REGISTRY_HTTP_ADDR="0.0.0.0:5000"
export REGISTRY_STORAGE_FILESYSTEM_ROOTDIRECTORY="/tmp/registry"
export REGISTRY_DATABASE_URL="postgresql://username:password@localhost/registry_db"
export REGISTRY_REDIS_URL="redis://localhost:6379"

# Run the registry
cargo run --bin registry
```

Or using Docker:

```dockerfile
FROM rust:1.80-alpine AS builder

RUN apk add --no-cache musl-dev postgresql-dev

WORKDIR /app

COPY rust-registry/Cargo.toml rust-registry/Cargo.lock ./

RUN mkdir src && echo 'fn main() {}' > src/main.rs

RUN cargo build --release

RUN rm -rf src

COPY rust-registry/src ./src

RUN cargo build --release

FROM alpine:latest

RUN apk add --no-cache ca-certificates

RUN addgroup -g 1001 -S appuser && \
    adduser -u 1001 -S appuser -G appuser

COPY --from=builder /app/target/release/registry /usr/local/bin/registry

RUN mkdir -p /tmp/registry && chown -R appuser:appuser /tmp/registry

USER appuser

EXPOSE 5000

CMD ["registry"]
```

## Building and Pushing Images

Currently, hyperbuild can build container images but doesn't have built-in push functionality. To integrate with hyperregistry, you would need to extend hyperbuild with the following capabilities:

### 1. Extend Hyperbuild with Push Functionality

The hyperbuild tool would need to be extended with functionality to:

- Convert the built image to OCI format
- Implement the Docker Registry HTTP API v2 protocol
- Handle authentication if required
- Push layers and manifests to hyperregistry

### 2. Example Implementation Concept

```rust
// In hyperbuild's src/push.rs (conceptual)
use oci_spec::image::{ImageManifest, ImageConfiguration};
use reqwest;
use anyhow::Result;

pub struct RegistryClient {
    client: reqwest::Client,
    registry_url: String,
}

impl RegistryClient {
    pub fn new(registry_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            registry_url,
        }
    }

    pub async fn push_image(&self, image: &Image) -> Result<()> {
        // Upload each layer
        for layer in &image.layers {
            self.upload_layer(layer).await?;
        }

        // Upload config
        let config_digest = self.upload_config(&image.config).await?;

        // Upload manifest
        self.upload_manifest(&image.manifest, &image.name).await?;

        Ok(())
    }

    async fn upload_layer(&self, layer: &Layer) -> Result<()> {
        // Implement layer upload using Docker Registry HTTP API
        // POST /v2/<name>/blobs/uploads/
        // PUT /v2/<name>/blobs/uploads/<uuid>?digest=<digest>
        todo!()
    }

    async fn upload_config(&self, config: &ImageConfiguration) -> Result<String> {
        // Upload image config and return its digest
        todo!()
    }

    async fn upload_manifest(&self, manifest: &ImageManifest, name: &str) -> Result<()> {
        // Upload manifest using Docker Registry HTTP API
        // PUT /v2/<name>/manifests/<reference>
        todo!()
    }
}
```

### 3. Updated Hyperbuild Command

With push functionality, the workflow would be:

```bash
# Build and push an image to hyperregistry
hyperbuild -c /path/to/context -f /path/to/Dockerfile -i myregistry.local:5000/my-app:latest --push
```

## Docker Compose Example

For easier development and testing, here's a docker-compose.yml file:

```yaml
version: '3.8'

services:
  registry-db:
    image: postgres:15
    environment:
      POSTGRES_DB: registry
      POSTGRES_USER: registry
      POSTGRES_PASSWORD: changeme
    volumes:
      - registry-db-data:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U registry"]
      interval: 10s
      timeout: 5s
      retries: 5

  registry-redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"

  hyperregistry:
    build:
      context: ./hyperregistry/rust-registry
      dockerfile: ../Containerfile.registry
    depends_on:
      registry-db:
        condition: service_healthy
      registry-redis:
        condition: service_started
    environment:
      - REGISTRY_HTTP_ADDR=0.0.0.0:5000
      - REGISTRY_STORAGE_FILESYSTEM_ROOTDIRECTORY=/tmp/registry
      - REGISTRY_DATABASE_URL=postgresql://registry:changeme@registry-db:5432/registry
      - REGISTRY_REDIS_URL=redis://registry-redis:6379
    ports:
      - "5000:5000"
    volumes:
      - registry-storage:/tmp/registry

  hyperbuild:
    build:
      context: ./hyperbuild
      dockerfile: Containerfile
    volumes:
      - ./examples:/workspace/examples
    depends_on:
      - hyperregistry

volumes:
  registry-db-data:
  registry-storage:
```

## Testing the Integration

1. Start the registry:
   ```bash
   cd hyperregistry/rust-registry
   docker-compose up -d hyperregistry
   ```

2. Build an image with hyperbuild:
   ```bash
   cd hyperbuild/rust-container-builder
   cargo run -- -c /path/to/context -f /path/to/Dockerfile -i localhost:5000/my-app:latest
   ```

3. Push the image (after implementing push functionality):
   ```bash
   # This command would work after implementing push functionality
   hyperbuild -c /path/to/context -f /path/to/Dockerfile -i localhost:5000/my-app:latest --push
   ```

4. Pull the image from the registry:
   ```bash
   docker pull localhost:5000/my-app:latest
   ```