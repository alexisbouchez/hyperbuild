#!/bin/bash

echo "Testing Hyperbuild + Hyperregistry Integration..."

# Build hyperbuild
echo "Building hyperbuild..."
cd /home/alexis-bouchez/Workspace/Projects/hyperbuild/rust-container-builder
cargo build

if [ $? -ne 0 ]; then
    echo "Failed to build hyperbuild"
    exit 1
fi

echo "Hyperbuild built successfully!"

# Test build command
echo "Testing build command..."
mkdir -p test-output
./target/debug/rust-container-builder build -c ../test-dockerfile -f ../test-dockerfile/Dockerfile -o test-output -i test-image:latest

if [ $? -eq 0 ]; then
    echo "Build command successful!"
else
    echo "Build command failed!"
    exit 1
fi

echo "Integration test completed successfully!"