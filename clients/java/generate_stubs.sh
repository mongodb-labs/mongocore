#!/bin/bash
# Generate Java gRPC stubs from proto files
# Normally handled by protobuf-maven-plugin during `mvn generate-sources`
set -e
echo "Java gRPC stubs are generated via Maven: mvn generate-sources"
echo "Ensure protobuf-maven-plugin is configured in pom.xml"
