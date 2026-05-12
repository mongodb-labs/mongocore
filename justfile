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

# Run all client integration tests
test-clients: test-python test-typescript test-go test-java

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

# Build release binary
release-local:
    cargo build --release
