# Contributing to litellm-rs

Thank you for your interest in contributing to litellm-rs!

## Development Setup

### Prerequisites

- Rust 1.87+ (install via [rustup](https://rustup.rs/))
- Docker and Docker Compose (for integration tests)

### Getting Started

```bash
# Clone the repository
git clone https://github.com/majiayu000/litellm-rs.git
cd litellm-rs

# Quick profile (recommended for most contributors)
make build
make test
make lint

# Standard profile (gateway/common bundle)
make build-standard
make test-standard
make lint-standard

# Full profile (heavy, release/nightly validation)
make build-full
make test-full
make lint-full

# Format code
cargo fmt --all
```

### Development Services

```bash
# Start PostgreSQL and Redis for development
make dev-services

# Stop services
make dev-stop
```

## Code Style

- Follow Rust standard formatting (`cargo fmt`)
- All code must pass `cargo clippy` without warnings
- Use meaningful variable and function names
- Add documentation comments for public APIs

## Commit Messages

We use [Conventional Commits](https://conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]

Signed-off-by: Your Name <email@example.com>
```

### Types

- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation only
- `style`: Code style (formatting, etc.)
- `refactor`: Code refactoring
- `perf`: Performance improvement
- `test`: Adding tests
- `chore`: Maintenance tasks

### Examples

```bash
git commit -m "feat(router): add weighted load balancing"
git commit -m "fix(auth): resolve JWT validation issue"
git commit -m "docs: update API examples"
```

## Pull Request Process

1. Fork the repository
2. Create a feature branch (`git checkout -b feat/my-feature`)
3. Make your changes
4. Run tests and lints
5. Commit with DCO sign-off (`git commit -s`)
6. Push and create a Pull Request

### DCO Sign-off

All commits must be signed off to certify you have the right to submit the code:

```bash
git commit -s -m "feat: add new feature"
```

This adds `Signed-off-by: Your Name <email>` to the commit.

## Testing

```bash
# Quick profile (lightweight default)
make test

# Standard gateway/common tests
make test-standard

# Full feature tests (heavy)
make test-full

# Run specific test (API-only profile)
cargo test test_name --no-default-features --features "lite"

# Run with logging (standard profile example)
RUST_LOG=debug cargo test --lib --tests --features "postgres sqlite redis s3 metrics tracing websockets analytics"
```

## Adding a New Provider

1. Create a new module in `src/core/providers/`
2. Implement the `Provider` trait
3. Register in `src/core/providers/mod.rs`
4. Add tests
5. Update documentation

## Questions?

- Open an issue for bugs or feature requests
- Check existing issues before creating new ones

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
