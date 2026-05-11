#!/bin/bash
# Generate TypeScript gRPC stubs from proto files
set -e

PROTO_DIR="../../proto"
OUT_DIR="src/generated"

mkdir -p "$OUT_DIR"

npx grpc_tools_node_protoc \
    -I "$PROTO_DIR" \
    --js_out=import_style=commonjs,binary:"$OUT_DIR" \
    --grpc_out=grpc_js:"$OUT_DIR" \
    --ts_out="$OUT_DIR" \
    mongocore/v1/types.proto \
    mongocore/v1/mongocore.proto

echo "TypeScript gRPC stubs generated in $OUT_DIR"
