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
