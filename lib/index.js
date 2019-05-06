const { spawn } = require("child_process");

module.exports = class Watcher {
  constructor() {
    this.nextRequestId = 0;
    this.nextWatchId = 0;
    this.watchCallbacks = new Map();
    this.pendingResponses = new Map();

    this.childProcess = spawn(
      `${__dirname}/../notify-subprocess-${process.platform}`,
      {
        stdio: ["pipe", "pipe", "pipe"]
      }
    );

    let stderr = "";
    this.childProcess.stderr.on("data", data => (stderr += data));
    this.childProcess.on("exit", code => {
      if (code) {
        console.error(`Notify subprocess exited with code ${code}:\n${stderr}`);
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

  async kill() {
    if (this.childProcess) {
      await this._sendRequest({ type: "unwatchAll" });
      this.childProcess.stdin.end();
      this.childProcess.kill();
      this.childProcess = null;
    }
  }

  async watchPath(path, callback) {
    if (!this.childProcess) {
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
    if (this.childProcess) {
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
    if (message.type === "ok") {
      this.pendingResponses.get(message.requestId).resolve();
      this.pendingResponses.delete(message.requestId);
    } else if (message.type === "error") {
      this.pendingResponses.get(message.requestId).reject(message.description);
      this.pendingResponses.delete(message.requestId);
    } else {
      const callback = this.watchCallbacks.get(message.watchId);
      if (callback) {
        callback(message.events);
      }
    }
  }
};
