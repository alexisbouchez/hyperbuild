# Configuration Examples for Hyperbuild and Hyperregistry

## Hyperbuild Configuration

Hyperbuild currently accepts command-line arguments for configuration. Here are the main options:

```bash
# Basic usage
hyperbuild -c /path/to/build/context -f /path/to/Dockerfile -i my-image:tag

# With verbose output
hyperbuild -c . -f Dockerfile -i my-image:tag -vv

# Specify output directory
hyperbuild -c . -f Dockerfile -i my-image:tag --output-dir ./build-output
```

## Hyperregistry Configuration

Hyperregistry uses environment variables for configuration. Here's a sample `.env` file:

```bash
# Server configuration
REGISTRY_HTTP_ADDR=0.0.0.0:5000
REGISTRY_HTTP_CORS_ALLOWED_ORIGINS=http://localhost:3000,https://mydomain.com

# Storage configuration
REGISTRY_STORAGE_TYPE=filesystem
REGISTRY_STORAGE_FILESYSTEM_ROOTDIRECTORY=/tmp/registry

# Database configuration
REGISTRY_DATABASE_URL=postgresql://registry_user:registry_password@localhost:5432/container_registry
REGISTRY_DATABASE_MAX_CONNECTIONS=10
REGISTRY_DATABASE_MIN_CONNECTIONS=1

# Redis configuration (optional, for caching and rate limiting)
REGISTRY_REDIS_URL=redis://localhost:6379
REGISTRY_REDIS_CACHE_TTL=3600
REGISTRY_REDIS_RATE_LIMITER_TTL=300

# Garbage collection configuration
REGISTRY_GC_ENABLED=true
REGISTRY_GC_INTERVAL=3600
REGISTRY_GC_DRY_RUN=false

# Authentication (JWT-based, optional)
# REGISTRY_AUTH_JWT_REALM=https://auth.mydomain.com
# REGISTRY_AUTH_JWT_ISSUER=registry.mydomain.com
# REGISTRY_AUTH_JWT_SERVICE=my-registry-service
# REGISTRY_AUTH_JWT_ROOTCERTBUNDLE=/path/to/ca.crt

# Feature flags
REGISTRY_FEATURES_TAG_DELETE=true
REGISTRY_FEATURES_MANIFEST_LIST_SUPPORT=true
REGISTRY_FEATURES_MULTI_ARCH_SUPPORT=true
REGISTRY_FEATURES_RATE_LIMITING=false
REGISTRY_FEATURES_SECURITY_SCANNING=false
REGISTRY_FEATURES_VULNERABILITY_REPORTING=false
```

## Docker Compose Setup

Here's a complete docker-compose.yml file that sets up both hyperbuild and hyperregistry:

```yaml
version: '3.8'

services:
  registry-db:
    image: postgres:15
    environment:
      POSTGRES_DB: container_registry
      POSTGRES_USER: registry_user
      POSTGRES_PASSWORD: registry_password
    volumes:
      - registry_db_data:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U registry_user"]
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
      dockerfile: ../Containerfile
    depends_on:
      registry-db:
        condition: service_healthy
      registry-redis:
        condition: service_started
    environment:
      - REGISTRY_HTTP_ADDR=0.0.0.0:5000
      - REGISTRY_STORAGE_FILESYSTEM_ROOTDIRECTORY=/tmp/registry
      - REGISTRY_DATABASE_URL=postgresql://registry_user:registry_password@registry-db:5432/container_registry
      - REGISTRY_REDIS_URL=redis://registry-redis:6379
      - REGISTRY_GC_ENABLED=true
      - REGISTRY_FEATURES_TAG_DELETE=true
      - REGISTRY_FEATURES_MANIFEST_LIST_SUPPORT=true
    ports:
      - "5000:5000"
    volumes:
      - registry_storage:/tmp/registry

  hyperbuild:
    build:
      context: ./hyperbuild/rust-container-builder
      dockerfile: ../Containerfile
    volumes:
      - ./examples:/workspace/examples
      - build_output:/workspace/build-output
    depends_on:
      - hyperregistry
    command: >
      sh -c "
        echo 'Hyperbuild container started.' &&
        echo 'To build an image, run:' &&
        echo 'hyperbuild -c /workspace/examples/my-app -f /workspace/examples/my-app/Dockerfile -i hyperregistry:5000/my-app:latest' &&
        sleep infinity
      "

volumes:
  registry_db_data:
  registry_storage:
  build_output:
```

## Example Usage Workflow

1. Start the infrastructure:
   ```bash
   docker-compose up -d
   ```

2. Build an image with hyperbuild (inside the hyperbuild container):
   ```bash
   docker exec -it <hyperbuild-container> hyperbuild -c /workspace/examples/my-app -f /workspace/examples/my-app/Dockerfile -i localhost:5000/my-app:latest
   ```

3. Push the image to hyperregistry (after implementing push functionality):
   ```bash
   # This would be implemented as part of hyperbuild
   # hyperbuild -c /workspace/examples/my-app -f /workspace/examples/my-app/Dockerfile -i localhost:5000/my-app:latest --push
   ```

4. Pull the image from hyperregistry:
   ```bash
   docker pull localhost:5000/my-app:latest
   ```

## Extending Hyperbuild for Registry Push

To enable pushing images to hyperregistry, hyperbuild would need to implement the Docker Registry HTTP API v2. Here's a conceptual example of how this could be added:

```rust
// In hyperbuild's main.rs, add a push command
use clap::Args;

#[derive(Args)]
struct PushCommand {
    /// Name of the image to push
    #[arg(short, long)]
    image_name: String,
    
    /// Registry URL to push to
    #[arg(long, default_value = "localhost:5000")]
    registry: String,
}

// Add to Args enum:
// #[command(subcommand)]
// push: Option<PushCommand>,
```

This would allow users to run:
```bash
hyperbuild push -i localhost:5000/my-app:latest
```