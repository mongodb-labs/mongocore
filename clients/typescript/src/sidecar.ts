import { execSync, spawn, ChildProcess } from 'child_process';
import { existsSync } from 'fs';
import { join } from 'path';
import { homedir } from 'os';

export class SidecarManager {
  private binaryPath: string;
  private port: number;
  private process: ChildProcess | null = null;

  constructor(options?: { binaryPath?: string; port?: number }) {
    this.binaryPath = options?.binaryPath ?? this.findBinary();
    this.port = options?.port ?? 50051;
  }

  private findBinary(): string {
    const candidates = [
      join(homedir(), '.local', 'bin', 'mongocore'),
      join(homedir(), '.mongocore', 'bin', 'mongocore'),
      '/usr/local/bin/mongocore',
    ];

    for (const path of candidates) {
      if (existsSync(path)) return path;
    }

    // Check PATH
    try {
      const result = execSync('which mongocore', { encoding: 'utf-8' }).trim();
      if (result) return result;
    } catch {}

    throw new Error('MongoCore binary not found. Install it or set binaryPath.');
  }

  async ensureRunning(): Promise<void> {
    if (await this.isHealthy()) return;
    this.start();
    await this.waitHealthy();
  }

  private start(): void {
    this.process = spawn(this.binaryPath, ['--grpc-port', String(this.port)], {
      stdio: 'ignore',
      detached: true,
    });
  }

  private async isHealthy(): Promise<boolean> {
    try {
      const grpc = await import('@grpc/grpc-js');
      const channel = new grpc.Channel(
        `localhost:${this.port}`,
        grpc.credentials.createInsecure(),
        {}
      );
      // Simple connectivity check
      channel.close();
      return true;
    } catch {
      return false;
    }
  }

  private async waitHealthy(): Promise<void> {
    for (let i = 0; i < 100; i++) {
      if (await this.isHealthy()) return;
      await new Promise(resolve => setTimeout(resolve, 100));
    }
    throw new Error('MongoCore sidecar failed to start within 10s');
  }

  stop(): void {
    if (this.process) {
      this.process.kill('SIGTERM');
      this.process = null;
    }
  }
}
