"""Sidecar binary management for dev mode."""

import os
import subprocess
import platform
import asyncio
from pathlib import Path


class SidecarManager:
    """Manages the MongoCore sidecar binary lifecycle."""

    BINARY_NAME = "mongocore"
    DEFAULT_PORT = 50051
    HEALTH_TIMEOUT = 10  # seconds

    def __init__(self, binary_path: str | None = None, port: int = DEFAULT_PORT):
        self._binary_path = binary_path or self._find_binary()
        self._port = port
        self._process = None

    def _find_binary(self) -> str:
        """Find the mongocore binary in PATH or default locations."""
        # Check PATH
        import shutil
        found = shutil.which(self.BINARY_NAME)
        if found:
            return found

        # Check common install locations
        home = Path.home()
        candidates = [
            home / ".local" / "bin" / self.BINARY_NAME,
            home / ".mongocore" / "bin" / self.BINARY_NAME,
            Path("/usr/local/bin") / self.BINARY_NAME,
        ]
        for path in candidates:
            if path.exists():
                return str(path)

        raise FileNotFoundError(
            f"MongoCore binary not found. Install it or set binary_path."
        )

    async def ensure_running(self):
        """Start the sidecar if it's not already running."""
        if await self._is_healthy():
            return

        self._start()
        await self._wait_healthy()

    def _start(self):
        """Spawn the sidecar process."""
        self._process = subprocess.Popen(
            [self._binary_path, "--grpc-port", str(self._port)],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )

    async def _is_healthy(self) -> bool:
        """Check if the sidecar is responding."""
        import grpc
        try:
            channel = grpc.aio.insecure_channel(f"localhost:{self._port}")
            await asyncio.wait_for(channel.channel_ready(), timeout=1.0)
            await channel.close()
            return True
        except Exception:
            return False

    async def _wait_healthy(self):
        """Wait for the sidecar to become healthy."""
        for _ in range(self.HEALTH_TIMEOUT * 10):
            if await self._is_healthy():
                return
            await asyncio.sleep(0.1)
        raise TimeoutError("MongoCore sidecar failed to start")

    def stop(self):
        """Stop the sidecar process."""
        if self._process:
            self._process.terminate()
            self._process.wait(timeout=5)
            self._process = None
