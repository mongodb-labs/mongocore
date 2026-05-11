#!/bin/bash
# Generate Python gRPC stubs from proto files
set -e

PROTO_DIR="../../proto"
OUT_DIR="src/mongocore/generated"

python -m grpc_tools.protoc \
    -I "$PROTO_DIR" \
    --python_out="$OUT_DIR" \
    --grpc_python_out="$OUT_DIR" \
    --pyi_out="$OUT_DIR" \
    mongocore/v1/types.proto \
    mongocore/v1/mongocore.proto

echo "Python gRPC stubs generated in $OUT_DIR"
