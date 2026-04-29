# Rust LiteLLM Gateway Makefile
# Provides convenient commands for development and deployment

.PHONY: help build build-standard build-full test test-standard test-full clean dev prod docker docs lint lint-standard lint-full format check check-standard check-full install deps start

# -----------------------------------------------------------------------------
# Build/Test profiles
# -----------------------------------------------------------------------------
# API-first defaults (for most users who use this as a unified API library)
API_FEATURES ?= lite
# Gateway/common CI bundle
STANDARD_FEATURES ?= postgres sqlite redis s3 metrics tracing websockets analytics
DEV_BUILD_JOBS ?= 4
DEV_TEST_THREADS ?= 4

# Default target
help: ## Show this help message
	@echo "Rust LiteLLM Gateway - Available Commands:"
	@echo ""
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2}'
	@echo ""
	@echo "Examples:"
	@echo "  make build        # API-only build (lightweight default)"
	@echo "  make test         # API-only tests (lightweight default)"
	@echo "  make test-standard # Gateway/common test bundle"
	@echo "  make test-full    # Full feature test (heavy)"

# =============================================================================
# DEVELOPMENT
# =============================================================================

start: ## Quick start (auto-loads config/gateway.yaml)
	@echo "🚀 Starting Rust LiteLLM Gateway..."
	cargo run

dev: deps dev-services ## Start development environment
	@echo "Starting development server..."
	RUST_LOG=debug cargo run --bin gateway -- --config config/dev.yaml

dev-services: ## Start development services (PostgreSQL, Redis, etc.)
	@echo "Starting development services..."
	docker-compose -f docker-compose.dev.yml up -d postgres-dev redis-dev
	@echo "Waiting for services to be ready..."
	@sleep 5

dev-stop: ## Stop development services
	@echo "Stopping development services..."
	docker-compose -f docker-compose.dev.yml down

dev-logs: ## Show development services logs
	docker-compose -f docker-compose.dev.yml logs -f

# =============================================================================
# BUILDING
# =============================================================================

build: ## Build API-only mode (lightweight default)
	CARGO_BUILD_JOBS=$(DEV_BUILD_JOBS) cargo build --lib --no-default-features --features "$(API_FEATURES)"

build-standard: ## Build standard gateway/common bundle
	CARGO_BUILD_JOBS=$(DEV_BUILD_JOBS) cargo build --features "$(STANDARD_FEATURES)"

build-full: ## Build with all features (heavy)
	CARGO_BUILD_JOBS=$(DEV_BUILD_JOBS) cargo build --all-features

build-release: ## Build optimized release version
	cargo build --release --all-features

prod: build-release ## Build production release (alias for build-release)

install: build-release ## Install binaries to system
	cargo install --path . --force

# =============================================================================
# TESTING
# =============================================================================

test: ## Run API-only tests (lightweight default)
	CARGO_BUILD_JOBS=$(DEV_BUILD_JOBS) cargo test --lib --tests --no-default-features --features "$(API_FEATURES)" -- --test-threads=$(DEV_TEST_THREADS)

test-standard: ## Run standard gateway/common test bundle
	CARGO_BUILD_JOBS=$(DEV_BUILD_JOBS) cargo test --lib --tests --features "$(STANDARD_FEATURES)" -- --test-threads=$(DEV_TEST_THREADS)

test-full: ## Run full feature test suite (heavy)
	CARGO_BUILD_JOBS=$(DEV_BUILD_JOBS) cargo test --workspace --all-features -- --test-threads=$(DEV_TEST_THREADS)

test-unit: ## Run API-only unit tests
	CARGO_BUILD_JOBS=$(DEV_BUILD_JOBS) cargo test --lib --no-default-features --features "$(API_FEATURES)" -- --test-threads=$(DEV_TEST_THREADS)

test-integration: ## Run integration tests with standard features
	CARGO_BUILD_JOBS=$(DEV_BUILD_JOBS) cargo test --test lib --features "$(STANDARD_FEATURES)" -- --test-threads=$(DEV_TEST_THREADS)

test-coverage: ## Generate test coverage report
	cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info
	@echo "Coverage report generated: lcov.info"

bench: ## Run benchmarks
	cargo bench --all-features

# =============================================================================
# CODE QUALITY
# =============================================================================

lint: ## Run clippy for API-only mode (lightweight default)
	CARGO_BUILD_JOBS=$(DEV_BUILD_JOBS) cargo clippy --lib --tests --no-default-features --features "$(API_FEATURES)" -- -D warnings

lint-standard: ## Run clippy for standard gateway/common bundle
	CARGO_BUILD_JOBS=$(DEV_BUILD_JOBS) cargo clippy --all-targets --features "$(STANDARD_FEATURES)" -- -D warnings

lint-full: ## Run clippy with all features (heavy)
	CARGO_BUILD_JOBS=$(DEV_BUILD_JOBS) cargo clippy --all-targets --all-features -- -D warnings

format: ## Format code with rustfmt
	cargo fmt --all

format-check: ## Check code formatting
	cargo fmt --all -- --check

check: ## Run cargo check for API-only mode (lightweight default)
	CARGO_BUILD_JOBS=$(DEV_BUILD_JOBS) cargo check --lib --tests --no-default-features --features "$(API_FEATURES)"

check-standard: ## Run cargo check for standard gateway/common bundle
	CARGO_BUILD_JOBS=$(DEV_BUILD_JOBS) cargo check --features "$(STANDARD_FEATURES)"

check-full: ## Run cargo check with all features (heavy)
	CARGO_BUILD_JOBS=$(DEV_BUILD_JOBS) cargo check --all-features

audit: ## Run security audit
	cargo audit

fix: ## Auto-fix code issues
	CARGO_BUILD_JOBS=$(DEV_BUILD_JOBS) cargo clippy --lib --tests --no-default-features --features "$(API_FEATURES)" --fix --allow-dirty
	cargo fmt --all

# =============================================================================
# DOCUMENTATION
# =============================================================================

docs: ## Generate and open documentation
	cargo doc --all-features --open

docs-build: ## Build documentation without opening
	cargo doc --all-features --no-deps

# =============================================================================
# DOCKER
# =============================================================================

docker: ## Build Docker image using optimized build script
	@echo "🐳 Building Docker image..."
	@./deployment/docker/build.sh

docker-dev: ## Build development Docker image
	@echo "🐳 Building development Docker image..."
	@./deployment/docker/build.sh -t dev

docker-run: ## Run Docker container
	@echo "🐳 Running Docker container..."
	docker run -p 8000:8000 -p 9090:9090 -v ./config:/app/config litellm-rs:latest

docker-run-dev: ## Run development Docker container
	@echo "🐳 Running development Docker container..."
	docker run -p 8000:8000 -p 9090:9090 -v ./config:/app/config litellm-rs:dev

docker-shell: ## Access Docker container shell
	docker run -it --entrypoint /bin/bash litellm-rs:latest

docker-compose: ## Start full stack with docker-compose
	docker-compose up -d

docker-compose-dev: ## Start development stack
	docker-compose -f docker-compose.dev.yml up -d

docker-compose-down: ## Stop docker-compose stack
	docker-compose down

docker-clean: ## Clean Docker images and containers
	docker system prune -f
	docker image prune -f

docker-tag: ## Tag image for registry (usage: make docker-tag TAG=v1.0.0)
	@if [ -z "$(TAG)" ]; then echo "❌ Please specify TAG: make docker-tag TAG=v1.0.0"; exit 1; fi
	docker tag litellm-rs:latest litellm-rs:$(TAG)
	@echo "✅ Tagged image as litellm-rs:$(TAG)"

# =============================================================================
# DATABASE
# =============================================================================

db-migrate: ## Run database migrations
	cargo run --bin gateway -- --config config/dev.yaml --migrate

db-reset: ## Reset development database
	docker-compose -f docker-compose.dev.yml down postgres-dev
	docker volume rm litellm_postgres_dev_data || true
	docker-compose -f docker-compose.dev.yml up -d postgres-dev
	@sleep 5
	$(MAKE) db-migrate

db-backup: ## Backup development database
	@mkdir -p backups
	docker exec litellm-postgres-dev pg_dump -U gateway_dev gateway_dev > backups/dev_backup_$(shell date +%Y%m%d_%H%M%S).sql

# =============================================================================
# DEPENDENCIES
# =============================================================================

deps: ## Install development dependencies
	@echo "Installing Rust toolchain components..."
	rustup component add rustfmt clippy llvm-tools-preview
	@echo "Installing cargo tools..."
	cargo install cargo-audit cargo-llvm-cov || true
	@echo "Dependencies installed!"

deps-update: ## Update dependencies
	cargo update

deps-outdated: ## Check for outdated dependencies
	cargo outdated || echo "Install cargo-outdated: cargo install cargo-outdated"

# =============================================================================
# DEPLOYMENT
# =============================================================================

deploy-staging: ## Deploy to staging environment
	@echo "Deploying to staging..."
	# Add your staging deployment commands here

deploy-prod: ## Deploy to production environment
	@echo "Deploying to production..."
	# Add your production deployment commands here

k8s-apply: ## Apply Kubernetes manifests
	kubectl apply -f deploy/kubernetes/

k8s-delete: ## Delete Kubernetes resources
	kubectl delete -f deploy/kubernetes/

# =============================================================================
# UTILITIES
# =============================================================================

clean: ## Clean build artifacts
	cargo clean
	docker system prune -f

clean-all: clean ## Clean everything including Docker volumes
	docker-compose down -v
	docker-compose -f docker-compose.dev.yml down -v

version: ## Show version information
	@echo "Rust version: $(shell rustc --version)"
	@echo "Cargo version: $(shell cargo --version)"
	@echo "Gateway version: $(shell cargo metadata --format-version 1 | jq -r '.packages[] | select(.name == "litellm-rs") | .version')"

health: ## Check gateway health
	@curl -s http://localhost:8000/health | jq . || echo "Gateway not running or jq not installed"

logs: ## Show gateway logs (if running in Docker)
	docker-compose logs -f gateway

sync-models: ## Fetch latest models from OpenRouter and generate report
	@echo "Fetching latest models from OpenRouter..."
	@python3 scripts/sync_models.py

sync-models-json: ## Fetch latest models as JSON
	@python3 scripts/sync_models.py --output json

sync-models-provider: ## Fetch models for specific provider (usage: make sync-models-provider PROVIDER=openai)
	@python3 scripts/sync_models.py --provider $(PROVIDER)

sync-models-update: ## Update data/model_prices.json with latest from OpenRouter
	@echo "Updating model prices..."
	@python3 scripts/sync_models.py --output update
	@echo "Done! Review changes in data/model_prices.json"

sync-models-rust: ## Generate Rust code snippets for models (usage: make sync-models-rust PROVIDER=deepseek)
	@python3 scripts/sync_models.py --output rust --provider $(PROVIDER)

# =============================================================================
# RELEASE
# =============================================================================

release-check: format-check lint-full test-full ## Run all checks before release
	@echo "All checks passed! Ready for release."

release-build: ## Build release artifacts for all platforms
	@echo "Building release artifacts..."
	cargo build --release --target x86_64-unknown-linux-gnu --all-features
	cargo build --release --target x86_64-unknown-linux-musl --all-features
	cargo build --release --target x86_64-pc-windows-msvc --all-features
	cargo build --release --target x86_64-apple-darwin --all-features

# =============================================================================
# MONITORING
# =============================================================================

metrics: ## Show Prometheus metrics
	@curl -s http://localhost:9090/metrics | head -20

grafana: ## Open Grafana dashboard
	@open http://localhost:3000 || echo "Grafana not running or 'open' command not available"

jaeger: ## Open Jaeger UI
	@open http://localhost:16686 || echo "Jaeger not running or 'open' command not available"

# =============================================================================
# CONFIGURATION
# =============================================================================

config-validate: ## Validate configuration
	cargo run --bin gateway -- --config config/dev.yaml --validate

config-example: ## Copy example configurations
	cp config/gateway.yaml.example config/gateway.yaml
	cp .env.example .env
	@echo "Example configurations copied. Please edit them with your values."

# =============================================================================
# VARIABLES
# =============================================================================

# Detect OS for platform-specific commands
UNAME_S := $(shell uname -s)
ifeq ($(UNAME_S),Linux)
    OPEN_CMD = xdg-open
endif
ifeq ($(UNAME_S),Darwin)
    OPEN_CMD = open
endif

# Default values
RUST_LOG ?= info
DATABASE_URL ?= postgresql://gateway_dev:dev_password@localhost:5433/gateway_dev
REDIS_URL ?= redis://localhost:6380
