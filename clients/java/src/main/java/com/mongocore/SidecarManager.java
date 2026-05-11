package com.mongocore;

import java.io.File;
import java.io.IOException;
import java.util.concurrent.TimeUnit;

public class SidecarManager {
    private static final int DEFAULT_PORT = 50051;
    private static final int HEALTH_TIMEOUT_SECONDS = 10;

    private final String binaryPath;
    private final int port;
    private Process process;

    public SidecarManager() {
        this(findBinary(), DEFAULT_PORT);
    }

    public SidecarManager(String binaryPath, int port) {
        this.binaryPath = binaryPath;
        this.port = port;
    }

    private static String findBinary() {
        String home = System.getProperty("user.home");
        String[] candidates = {
            home + "/.local/bin/mongocore",
            home + "/.mongocore/bin/mongocore",
            "/usr/local/bin/mongocore",
        };

        for (String path : candidates) {
            if (new File(path).exists()) {
                return path;
            }
        }

        // Check PATH
        String pathEnv = System.getenv("PATH");
        if (pathEnv != null) {
            for (String dir : pathEnv.split(File.pathSeparator)) {
                File candidate = new File(dir, "mongocore");
                if (candidate.exists() && candidate.canExecute()) {
                    return candidate.getAbsolutePath();
                }
            }
        }

        throw new IllegalStateException("MongoCore binary not found. Install it or set binaryPath.");
    }

    public void start() throws IOException {
        ProcessBuilder pb = new ProcessBuilder(binaryPath, "--grpc-port", String.valueOf(port));
        pb.redirectOutput(ProcessBuilder.Redirect.DISCARD);
        pb.redirectError(ProcessBuilder.Redirect.DISCARD);
        process = pb.start();
    }

    public void waitReady() throws InterruptedException {
        long deadline = System.currentTimeMillis() + (HEALTH_TIMEOUT_SECONDS * 1000L);
        while (System.currentTimeMillis() < deadline) {
            Thread.sleep(100);
            // In production, would check gRPC health endpoint
        }
    }

    public void stop() {
        if (process != null) {
            process.destroy();
            try {
                process.waitFor(5, TimeUnit.SECONDS);
            } catch (InterruptedException e) {
                process.destroyForcibly();
            }
            process = null;
        }
    }
}
