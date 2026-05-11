# MongoCore task runner

# Run unit tests only
test-unit:
    cargo test --lib

# Run integration tests only (requires MongoDB running)
test-integration:
    cargo test --test integration

# Run all tests
test-all:
    cargo test

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
