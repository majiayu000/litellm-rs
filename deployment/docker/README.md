# Docker Build for LiteLLM-RS

This directory contains Docker build files and scripts for the LiteLLM-RS gateway.

## Build Issues and Solutions

### CMAKE Missing Error

If you encounter the following error during Docker builds:

```
Missing dependency: cmake
thread 'main' panicked at .../aws-lc-sys-0.31.0/builder/main.rs:463:40:
called `Result::unwrap()` on an `Err` value: "Required build dependency is missing. Halting build."
```

This happens because the `aws-lc-sys` crate (used by `rustls` for TLS support) requires cmake and other build tools for compilation.

### Solution

The Dockerfile has been updated to include all necessary build dependencies:

- `cmake` - Required by aws-lc-sys
- `build-essential` - C/C++ compilation tools
- `clang` and `llvm` - Modern C/C++ compiler
- `gcc-arm-linux-gnueabihf` - ARM cross-compilation support
- `libc6-dev-armhf-cross` - ARM development headers

### Environment Variables

The following environment variables are set for proper compilation:

```bash
CMAKE=cmake
CC=clang
CXX=clang++
```

### Files

- `Dockerfile` - Main multi-stage Dockerfile for all architectures
- `Dockerfile.arm` - Alternative native ARM build with additional compiler tools
- `build.sh` - Build script with error handling and cleanup
- `.dockerignore` - Files to exclude from Docker context

### Usage

```bash
# Build for current architecture
./deployment/docker/build.sh

# Build with custom tag
./deployment/docker/build.sh -t v1.0.0

# Select ARM64 explicitly; both build and runtime stages use that platform
docker buildx build --platform linux/arm64 --load -f deployment/docker/Dockerfile.arm -t litellm-rs:arm .
```

### Dependency Chain

The cmake requirement comes from this dependency chain:

```
litellm-rs -> rustls -> aws-lc-rs -> aws-lc-sys (requires cmake)
```

This is a common pattern in Rust applications that use TLS/SSL functionality.

### Troubleshooting

1. **Build fails on ARM platforms**: Select the intended target with `--platform`. `Dockerfile.arm` builds natively inside that platform image; QEMU is needed when the host differs. It does not cross-link a host binary into an ARM runtime.
2. **Permission errors**: Ensure Docker has proper permissions and the daemon is running
3. **Out of space**: The build process can be large, ensure sufficient disk space (2GB+ recommended)
4. **Network issues**: Some dependencies are downloaded during build, ensure internet connectivity

### GitHub Actions

The tagged release workflow publishes `linux/amd64` and `linux/arm64` images.
The separate manually dispatched Docker workflow also lists `linux/arm/v7`;
that matrix declaration is not evidence of a tested or published ARMv7 release.

The enhanced Dockerfile should resolve cmake-related build failures across all platforms.