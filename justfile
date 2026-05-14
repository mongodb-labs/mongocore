# MongoCore task runner

# Run unit tests only
test-unit:
    cargo test --lib

# Run integration tests only (requires MongoDB running)
test-integration:
    cargo test --test integration

# Run all Rust tests
test-rust:
    cargo test

# Run Python client tests (unit + integration)
test-python:
    cd clients/python && python3 -m pytest tests/ -v

# Run TypeScript client tests (unit + integration)
test-typescript:
    cd clients/typescript && npx jest --no-coverage

# Run Go client tests (unit + integration)
test-go:
    cd clients/go && go test ./mongocore/ -v -count=1

# Run Java client tests (unit + integration)
test-java:
    cd clients/java && mvn test

# Run all client tests (starts/stops sidecar automatically, requires Docker MongoDB running)
test-clients:
    #!/usr/bin/env bash
    set -e
    # Kill any existing sidecar on port 50051
    EXISTING_PID=$(lsof -ti :50051 -sTCP:LISTEN 2>/dev/null || true)
    if [ -n "$EXISTING_PID" ]; then
        echo "Killing existing sidecar (PID $EXISTING_PID)..."
        kill $EXISTING_PID 2>/dev/null || true
        sleep 1
    fi
    cargo build --release
    ./target/release/mongocore --connection-uri "mongodb://localhost:27017" &
    SIDECAR_PID=$!
    trap "kill $SIDECAR_PID 2>/dev/null; wait $SIDECAR_PID 2>/dev/null || true" EXIT
    # Wait for gRPC port to be ready
    for i in $(seq 1 30); do
        if lsof -i :50051 -sTCP:LISTEN > /dev/null 2>&1; then
            echo "MongoCore sidecar ready (PID $SIDECAR_PID)"
            break
        fi
        sleep 1
    done
    if ! lsof -i :50051 -sTCP:LISTEN > /dev/null 2>&1; then
        echo "ERROR: Sidecar failed to start within 30s"
        exit 1
    fi
    # Run all client tests
    echo "====================="
    echo " PYTHON test suite"
    echo "====================="
    cd clients/python && python3 -m pytest tests/ -v && cd ../..
    echo ""
    echo "====================="
    echo " TYPESCRIPT test suite"
    echo "====================="
    cd clients/typescript && npx jest --no-coverage && cd ../..
    echo ""
    echo "====================="
    echo " GO test suite"
    echo "====================="
    cd clients/go && go test ./mongocore/ -v -count=1 && cd ../..
    echo ""
    echo "====================="
    echo " JAVA test suite"
    echo "====================="
    cd clients/java && mvn test && cd ../..
    echo ""
    echo "==================================="
    echo "      ALL CLIENT TESTS PASSED"
    echo "==================================="

# Run Python client unit tests
test-unit-python:
    cd clients/python && python3 -m pytest tests/test_client.py -v

# Run TypeScript client unit tests
test-unit-typescript:
    cd clients/typescript && npx jest tests/unit.test.ts --no-coverage

# Run Go client unit tests
test-unit-go:
    cd clients/go && go test ./mongocore/ -v -count=1 -run "^TestUnit"

# Run Java client unit tests
test-unit-java:
    cd clients/java && mvn test -Dtest=MongoClientTest

# Run all client unit tests
test-unit-clients: test-unit-python test-unit-typescript test-unit-go test-unit-java

# Run all tests (Rust + all client tests)
test-all: test-rust test-clients

# Run compiled query LLM tests (requires TEST_LLM_INTEGRATION=true + LLM configured + sample data)
test-llm:
    TEST_LLM_INTEGRATION=true cargo test --test integration compiled_llm -- --nocapture

# Start MongoDB for testing
docker-up:
    docker compose -f docker-compose.test.yml up -d

# Stop MongoDB test container
docker-down:
    docker compose -f docker-compose.test.yml down

# Build Docker image
docker-build:
    docker build -t mongocore:dev .

# Run Docker container
docker-run:
    docker run --rm -p 50051:50051 -p 3000:3000 mongocore:dev

# Regenerate proto stubs for all client languages (requires protoc + language plugins)
proto-gen:
    #!/usr/bin/env bash
    set -e
    echo "=== Rust (via cargo build) ==="
    cargo build
    echo ""
    echo "=== Python ==="
    cd clients/python && python3 -m grpc_tools.protoc -I../../proto \
      --python_out=src/mongocore/generated --grpc_python_out=src/mongocore/generated \
      --pyi_out=src/mongocore/generated \
      ../../proto/mongocore/v1/mongocore.proto ../../proto/mongocore/v1/types.proto \
      ../../proto/mongocore/v1/ingestion.proto
    cd src/mongocore/generated/mongocore/v1 && sed -i '' 's/from mongocore\.v1 import/from . import/g' *.py
    echo "Python stubs generated (imports fixed)"
    cd ../../../../..
    echo ""
    echo "=== TypeScript ==="
    cd clients/typescript && bash generate_stubs.sh
    cd ../..
    echo ""
    echo "=== Go ==="
    cd clients/go && bash generate_stubs.sh
    cd ../..
    echo ""
    echo "=== Java ==="
    cd clients/java && bash generate_stubs.sh
    cd ../..
    echo ""
    echo "All proto stubs regenerated."

# Regenerate Python proto stubs only
proto-gen-python:
    #!/usr/bin/env bash
    set -e
    cd clients/python
    python3 -m grpc_tools.protoc -I../../proto \
      --python_out=src/mongocore/generated --grpc_python_out=src/mongocore/generated \
      --pyi_out=src/mongocore/generated \
      ../../proto/mongocore/v1/mongocore.proto ../../proto/mongocore/v1/types.proto \
      ../../proto/mongocore/v1/ingestion.proto
    # Fix absolute imports to relative (grpc_tools generates absolute but package needs relative)
    cd src/mongocore/generated/mongocore/v1
    sed -i '' 's/from mongocore\.v1 import/from . import/g' *.py
    echo "Python stubs generated (imports fixed)"

# Build release binary
release-local:
    cargo build --release
