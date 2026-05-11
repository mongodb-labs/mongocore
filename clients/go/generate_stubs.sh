#!/bin/bash
# Generate Go gRPC stubs from proto files
set -e

PROTO_DIR="../../proto"
OUT_DIR="./generated"

mkdir -p "$OUT_DIR"

protoc \
    -I "$PROTO_DIR" \
    --go_out="$OUT_DIR" \
    --go-grpc_out="$OUT_DIR" \
    --go_opt=paths=source_relative \
    --go-grpc_opt=paths=source_relative \
    mongocore/v1/types.proto \
    mongocore/v1/mongocore.proto

echo "Go gRPC stubs generated in $OUT_DIR"
