# Rust Container Builder

A Rust-based container builder inspired by BuildKit, designed to build OCI-compliant container images from Dockerfiles.

## Features

- **Dockerfile Parsing**: Parses Dockerfiles and understands common instructions (FROM, RUN, COPY, WORKDIR, CMD, etc.)
- **Multi-stage Build Support**: Supports multi-stage builds with named stages
- **OCI Compliance**: Generates OCI-compliant image manifests and configurations
- **Layer Management**: Creates separate layers for each Dockerfile instruction
- **Storage Management**: Efficient storage of layers and images with content-addressable storage

## Architecture

The project is organized into several modules:

- `dockerfile/`: Contains the Dockerfile parser that converts Dockerfile instructions into an AST
- `storage/`: Manages the storage of layers and images on disk
- `engine/`: Orchestrates the build process, applying Dockerfile instructions to create layers
- `main.rs`: Entry point that coordinates the build process

## How It Works

1. **Parse**: The Dockerfile parser reads and parses the Dockerfile into structured instructions
2. **Execute**: The build engine processes each instruction, creating a new layer for each
3. **Package**: Layers are packaged into an OCI-compliant image with proper manifest
4. **Store**: The image is stored in the output directory with all its layers

## Usage

```bash
# Build an image from a Dockerfile
cargo run -- -i my-image-name

# Build with verbose output
cargo run -- -i my-image-name -v

# Specify custom Dockerfile and context
cargo run -- -c /path/to/context -d /path/to/Dockerfile -i my-image-name
```

## Comparison to BuildKit

While this Rust implementation is much simpler than the full BuildKit, it shares similar concepts:

- Pluggable architecture
- Layer-based builds
- OCI compliance
- Efficient storage management

## Future Enhancements

- Real execution of RUN commands (currently simulated)
- Proper handling of build contexts and file copying
- Cache management and reuse
- Support for build arguments and environment variables
- Network isolation during builds
- More Dockerfile instructions