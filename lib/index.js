const { spawn } = require("child_process");

module.exports = class Watcher {
  constructor(params) {
    this.nextRequestId = 0;
    this.nextWatchId = 0;
    this.watchCallbacks = new Map();
    this.pendingResponses = new Map();
    this.killed = false;
    if (params && typeof params.onError === "function") {
      this.onError = params.onError;
    }

    const args = [];
    if (params.pollInterval) args.push("--poll-interval", params.pollInterval);
    this.childProcess = spawn(
      `${__dirname}/../notify-subprocess-${process.platform}`,
      args,
      {
        stdio: ["pipe", "pipe", "pipe"]
      }
    );
    this.kill = this.kill.bind(this);
    process.on("exit", this.kill);

    let stderr = "";
    this.childProcess.stderr.on("data", data => (stderr += data));
    this.childProcess.on("exit", code => {
      this.killed = true;
      if (code) {
        this.onError(`Notify subprocess exited with code ${code}:\n${stderr}`);
      }
    });

    let buffer = "";
    this.childProcess.stdout.on("data", chunk => {
      buffer += chunk;
      const lines = buffer.split("\n");
      buffer = lines.pop();
      for (const line of lines) {
        this._handleMessage(JSON.parse(line));
      }
    });
  }

  // This function can be overridden via the `onError` constructor parameter
  onError(error) {
    console.error(`Error watching for file system notifications: ${error}`);
  }

  async kill() {
    if (!this.killed) {
      process.removeListener("exit", this.kill);
      this.killed = true;
      await this._sendRequest({ type: "unwatchAll" });
      this.childProcess.stdin.end();
      this.childProcess.kill();
    }
  }

  async watchPath(path, callback) {
    if (this.killed) {
      throw new Error("This watcher has been killed");
    }

    const watchId = this.nextWatchId++;
    this.watchCallbacks.set(watchId, callback);
    await this._sendRequest({
      type: "watch",
      watchId: watchId,
      root: path
    });

    let disposed = false;
    return {
      dispose: async () => {
        if (!disposed) {
          disposed = true;
          await this._unwatch(watchId);
        }
      }
    };
  }

  async _unwatch(watchId) {
    if (!this.killed) {
      this.watchCallbacks.delete(watchId);
      await this._sendRequest({
        type: "unwatch",
        watchId: watchId
      });
    }
  }

  _sendRequest(message) {
    const requestId = this.nextRequestId++;
    const request = Object.assign({}, message, { requestId });
    this.childProcess.stdin.write(JSON.stringify(request) + "\n");
    return new Promise((resolve, reject) => {
      this.pendingResponses.set(requestId, { resolve, reject });
    });
  }

  _handleMessage(message) {
    if (message.type === "okResponse") {
      this.pendingResponses.get(message.requestId).resolve();
      this.pendingResponses.delete(message.requestId);
    } else if (message.type === "errorResponse") {
      this.pendingResponses.get(message.requestId).reject(message.description);
      this.pendingResponses.delete(message.requestId);
    } else if (message.type === "watchEvents") {
      const callback = this.watchCallbacks.get(message.watchId);
      if (callback) callback(message.events);
    } else if (message.type === "watcherError") {
      this.onError(message.description);
    } else {
      throw new Error(`Unexpected message type ${message.type}`);
    }
  }
};
