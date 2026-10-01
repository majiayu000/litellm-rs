#!/bin/bash

# Rust LiteLLM Gateway Startup Script
# This script provides a convenient way to start the gateway with proper configuration

set -euo pipefail

# Default values
CONFIG_FILE="${CONFIG_FILE:-config/gateway.yaml}"
LOG_LEVEL="${LOG_LEVEL:-info}"
ENVIRONMENT="${ENVIRONMENT:-production}"
HOST="${HOST:-0.0.0.0}"
PORT="${PORT:-8000}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Function to check prerequisites
check_prerequisites() {
    print_info "Checking prerequisites..."
    
    # Check if the gateway binary exists
    if [ ! -f "./target/release/gateway" ] && [ ! -f "./target/debug/gateway" ]; then
        print_error "Gateway binary not found. Please build the project first:"
        echo "  cargo build --release --bin gateway --features sqlite"
        exit 1
    fi
    
    # Check if config file exists
    if [ ! -f "$CONFIG_FILE" ]; then
        print_error "Configuration file not found: $CONFIG_FILE"
        echo "Please create a configuration file or set CONFIG_FILE environment variable."
        exit 1
    fi
    
    # Check if required environment variables are set
    if [ -z "${DATABASE_URL:-}" ]; then
        print_warning "DATABASE_URL not set. Using default from config file."
    fi
    
    if [ -z "${REDIS_URL:-}" ]; then
        print_warning "REDIS_URL not set. Using default from config file."
    fi
    
    if [ -z "${JWT_SECRET:-}" ]; then
        print_warning "JWT_SECRET not set. Using default from config file."
    fi
    
    print_success "Prerequisites check completed"
}

# Function to validate configuration
validate_config() {
    print_info "Validating configuration..."
    
    local binary_path
    if [ -f "./target/release/rust-litellm-gateway" ]; then
        binary_path="./target/release/rust-litellm-gateway"
    else
        binary_path="./target/debug/rust-litellm-gateway"
    fi
    
    if ! "$binary_path" validate-config --config "$CONFIG_FILE"; then
        print_error "Configuration validation failed"
        exit 1
    fi
    
    print_success "Configuration is valid"
}

# Function to start the gateway
start_gateway() {
    print_info "Starting LiteLLM-RS Gateway..."
    print_info "Configuration: $CONFIG_FILE"
    print_info "Environment: $ENVIRONMENT"
    print_info "Log Level: $LOG_LEVEL"
    print_info "Host: $HOST"
    print_info "Port: $PORT"

    local binary_path
    if [ -f "./target/release/gateway" ]; then
        binary_path="./target/release/gateway"
        print_info "Using release binary"
    else
        binary_path="./target/debug/gateway"
        print_info "Using debug binary"
    fi
    
    # Set environment variables
    export RUST_LOG="$LOG_LEVEL"
    export ENVIRONMENT="$ENVIRONMENT"
    
    # Start the gateway
    exec "$binary_path" \
        --config "$CONFIG_FILE" \
        --host "$HOST" \
        --port "$PORT" \
        --log-level "$LOG_LEVEL"
}

# Function to show usage
show_usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  -c, --config FILE     Configuration file path (default: config/gateway.yaml)"
    echo "  -l, --log-level LEVEL Log level (default: info)"
    echo "  -e, --env ENV         Environment (default: production)"
    echo "  -h, --host HOST       Server host (default: 0.0.0.0)"
    echo "  -p, --port PORT       Server port (default: 8000)"
    echo "  --validate-only       Only validate configuration, don't start"
    echo "  --help                Show this help message"
    echo ""
    echo "Environment Variables:"
    echo "  CONFIG_FILE           Configuration file path"
    echo "  LOG_LEVEL             Log level"
    echo "  ENVIRONMENT           Environment name"
    echo "  HOST                  Server host"
    echo "  PORT                  Server port"
    echo "  DATABASE_URL          Database connection URL"
    echo "  REDIS_URL             Redis connection URL"
    echo "  JWT_SECRET            JWT signing secret"
    echo ""
    echo "Examples:"
    echo "  $0                                    # Start with defaults"
    echo "  $0 -c config/dev.yaml -l debug       # Start with custom config and debug logging"
    echo "  $0 --validate-only                   # Only validate configuration"
    echo "  CONFIG_FILE=config/prod.yaml $0      # Start with environment variable"
}

# Parse command line arguments
VALIDATE_ONLY=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -c|--config)
            CONFIG_FILE="$2"
            shift 2
            ;;
        -l|--log-level)
            LOG_LEVEL="$2"
            shift 2
            ;;
        -e|--env)
            ENVIRONMENT="$2"
            shift 2
            ;;
        -h|--host)
            HOST="$2"
            shift 2
            ;;
        -p|--port)
            PORT="$2"
            shift 2
            ;;
        --validate-only)
            VALIDATE_ONLY=true
            shift
            ;;
        --help)
            show_usage
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            show_usage
            exit 1
            ;;
    esac
done

# Main execution
main() {
    print_info "Rust LiteLLM Gateway Startup Script"
    print_info "===================================="
    
    check_prerequisites
    validate_config
    
    if [ "$VALIDATE_ONLY" = true ]; then
        print_success "Configuration validation completed successfully"
        exit 0
    fi
    
    start_gateway
}

# Handle signals for graceful shutdown
trap 'print_info "Received shutdown signal, stopping gateway..."; exit 0' SIGTERM SIGINT

# Run main function
main "$@"
