#!/bin/bash

# 🚀 Rust LiteLLM Gateway 快速启动脚本

echo "🚀 启动 Rust LiteLLM Gateway"
echo "================================"

# 获取项目根目录 (脚本在 deployment/scripts/ 下)
PROJECT_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$PROJECT_ROOT"

echo "📁 项目目录: $PROJECT_ROOT"

# 检查配置文件
if [ ! -f "config/gateway.yaml" ]; then
    echo "❌ 配置文件不存在: config/gateway.yaml"
    echo "💡 请先创建配置文件并填入 API 密钥"
    echo ""
    echo "示例配置:"
    echo "  cp config/gateway.yaml.example config/gateway.yaml"
    echo "  nano config/gateway.yaml"
    exit 1
fi

echo "✅ 配置文件存在"
echo "🔧 编译并启动..."
echo ""

# 启动 Gateway
cargo run --bin gateway --features sqlite

echo ""
echo "👋 Gateway 已停止"
