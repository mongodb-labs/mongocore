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

# Run Python client integration tests
test-python:
    cd clients/python && python3 -m pytest tests/test_integration.py -v

# Run TypeScript client integration tests
test-typescript:
    cd clients/typescript && npx jest tests/integration.test.ts --no-coverage

# Run Go client integration tests
test-go:
    cd clients/go && go test ./mongocore/ -v -count=1

# Run Java client integration tests
test-java:
    cd clients/java && mvn test -Dtest=IntegrationTest -q

# Run all client integration tests
test-clients: test-python test-typescript test-go test-java

# Run all tests (Rust + client integrations)
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
