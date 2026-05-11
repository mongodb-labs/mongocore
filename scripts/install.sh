#!/bin/bash
# MongoCore installer
# Usage: curl -fsSL https://raw.githubusercontent.com/rozza/mongocore/main/scripts/install.sh | bash
set -e

VERSION="${MONGOCORE_VERSION:-latest}"
INSTALL_DIR="${MONGOCORE_INSTALL_DIR:-$HOME/.local/bin}"

OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

case "$ARCH" in
  x86_64|amd64) ARCH="amd64" ;;
  aarch64|arm64) ARCH="arm64" ;;
  *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

case "$OS" in
  linux) PLATFORM="linux" ;;
  darwin) PLATFORM="darwin" ;;
  *) echo "Unsupported OS: $OS"; exit 1 ;;
esac

BINARY="mongocore-${PLATFORM}-${ARCH}"

if [ "$VERSION" = "latest" ]; then
  DOWNLOAD_URL="https://github.com/rozza/mongocore/releases/latest/download/${BINARY}"
else
  DOWNLOAD_URL="https://github.com/rozza/mongocore/releases/download/${VERSION}/${BINARY}"
fi

echo "Installing MongoCore..."
echo "  Platform: ${PLATFORM}/${ARCH}"
echo "  Version: ${VERSION}"
echo "  Install directory: ${INSTALL_DIR}"

mkdir -p "$INSTALL_DIR"
curl -fsSL "$DOWNLOAD_URL" -o "${INSTALL_DIR}/mongocore"
chmod +x "${INSTALL_DIR}/mongocore"

echo ""
echo "MongoCore installed successfully!"
echo ""
echo "Make sure ${INSTALL_DIR} is in your PATH:"
echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
echo ""
echo "Get started:"
echo "  mongocore --help"
