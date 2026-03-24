import * as net from "net";

/**
 * RAP (MechGen Agent Protocol) client — JSON-RPC over TCP.
 *
 * Connects to the RAP server (prototype/src/rap.rs) and sends
 * line-delimited JSON-RPC requests.
 */
export class RapClient {
  private socket: net.Socket | undefined;
  private host: string;
  private port: number;
  private nextId = 1;
  private pending = new Map<number, { resolve: (v: any) => void; reject: (e: Error) => void }>();
  private buffer = "";

  constructor(address: string) {
    const [host, portStr] = address.split(":");
    this.host = host || "127.0.0.1";
    this.port = parseInt(portStr, 10) || 9876;
  }

  async connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      this.socket = net.createConnection({ host: this.host, port: this.port }, () => {
        resolve();
      });

      this.socket.on("error", (err) => {
        reject(err);
      });

      this.socket.on("data", (data) => {
        this.buffer += data.toString();
        this.processBuffer();
      });

      this.socket.on("close", () => {
        // Reject all pending requests.
        for (const [, entry] of this.pending) {
          entry.reject(new Error("Connection closed"));
        }
        this.pending.clear();
        this.socket = undefined;
      });
    });
  }

  disconnect(): void {
    if (this.socket) {
      this.socket.destroy();
      this.socket = undefined;
    }
  }

  async request(method: string, params: Record<string, unknown>): Promise<any> {
    if (!this.socket) {
      throw new Error("Not connected to RAP server");
    }

    const id = this.nextId++;
    const message = JSON.stringify({
      jsonrpc: "2.0",
      id,
      method,
      params,
    });

    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.socket!.write(message + "\n");
    });
  }

  private processBuffer(): void {
    let newlineIdx: number;
    while ((newlineIdx = this.buffer.indexOf("\n")) !== -1) {
      const line = this.buffer.slice(0, newlineIdx).trim();
      this.buffer = this.buffer.slice(newlineIdx + 1);

      if (!line) continue;

      try {
        const response = JSON.parse(line);
        const id = response.id;
        const entry = this.pending.get(id);
        if (entry) {
          this.pending.delete(id);
          if (response.error) {
            entry.reject(new Error(response.error.message ?? "RPC error"));
          } else {
            entry.resolve(response.result);
          }
        }
      } catch {
        // Malformed response — skip.
      }
    }
  }
}
