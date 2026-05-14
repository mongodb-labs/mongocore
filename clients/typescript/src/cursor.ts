import { BSON } from 'bson';
import { CLIENT_METADATA } from './client';
import type { Document } from './types';

export class Cursor implements AsyncIterable<Document> {
  private grpcClient: any;
  private request: any;
  private rpcMethod: string;
  private stream: any = null;
  private buffer: Document[] = [];
  private bufferIndex: number = 0;
  private exhausted: boolean = false;
  private error: Error | null = null;

  constructor(grpcClient: any, request: any, rpcMethod: string) {
    this.grpcClient = grpcClient;
    this.request = request;
    this.rpcMethod = rpcMethod;
  }

  [Symbol.asyncIterator](): AsyncIterator<Document> {
    return {
      next: async (): Promise<IteratorResult<Document>> => {
        const doc = await this.nextDoc();
        if (doc === null) {
          return { done: true, value: undefined };
        }
        return { done: false, value: doc };
      },
      return: async (): Promise<IteratorResult<Document>> => {
        this.close();
        return { done: true, value: undefined };
      },
    };
  }

  private async nextDoc(): Promise<Document | null> {
    if (this.error) {
      throw this.error;
    }

    if (this.bufferIndex < this.buffer.length) {
      return this.buffer[this.bufferIndex++];
    }

    if (this.exhausted) {
      return null;
    }

    await this.fetchNextBatch();

    if (this.error) {
      throw this.error;
    }

    if (this.bufferIndex < this.buffer.length) {
      return this.buffer[this.bufferIndex++];
    }

    return null;
  }

  private fetchNextBatch(): Promise<void> {
    return new Promise((resolve, reject) => {
      if (!this.stream) {
        this.stream = this.grpcClient[this.rpcMethod](this.request, CLIENT_METADATA);
        this.stream.on('error', (err: any) => {
          this.error = err;
          this.exhausted = true;
          resolve();
        });
      }

      const onData = (batch: any) => {
        this.stream.removeListener('data', onData);
        this.stream.removeListener('end', onEnd);

        const docs = (batch.documents || []).map((d: any) =>
          BSON.deserialize(Buffer.from(d.data)) as Document
        );
        this.buffer = docs;
        this.bufferIndex = 0;

        if (!batch.hasMore) {
          this.exhausted = true;
        }
        resolve();
      };

      const onEnd = () => {
        this.stream.removeListener('data', onData);
        this.exhausted = true;
        resolve();
      };

      this.stream.once('data', onData);
      this.stream.once('end', onEnd);
    });
  }

  async toArray(): Promise<Document[]> {
    const results: Document[] = [];
    for await (const doc of this) {
      results.push(doc);
    }
    return results;
  }

  close(): void {
    if (this.stream) {
      this.stream.cancel();
      this.stream = null;
    }
    this.exhausted = true;
  }
}
