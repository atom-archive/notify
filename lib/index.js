const { spawn } = require("child_process");
const lineByLine = require("linebyline");

module.exports = class Supervisor {
  constructor(options = {}) {
    this.nextId = 0;
    this.watchCallbacks = new Map();
    this.pendingResponses = new Map();

    const env = {};
    if (options.mode === "debug") {
      env.RUST_BACKTRACE = 1;
    }

    this.childProcess = spawn(
      `${__dirname}/../target/${options.mode || "release"}/notify-subprocess`,
      {
        stdio: ["pipe", "pipe", "inherit"],
        env
      }
    );

    lineByLine(this.childProcess.stdout)
      .on("line", line => {
        this._handleMessage(JSON.parse(line));
      })
      .on("error", error => {
        console.error("Error reading child process stdout: ", error);
      });
  }

  kill() {
    this.childProcess.kill();
  }

  async watchPath(path, callback) {
    let id = this.nextId++;
    this.watchCallbacks.set(id, callback);
    await this._sendRequest({
      type: "watch",
      id: id,
      root: path
    });
    return {
      dispose: () => this._unwatch(id)
    };
  }

  _unwatch(id) {
    this.watchCallbacks.delete(id);
    this.sendRequest({
      type: "unwatch",
      id: id
    });
  }

  _sendRequest(message) {
    this.childProcess.stdin.write(JSON.stringify(message) + "\n");
    return new Promise((resolve, reject) => {
      this.pendingResponses.set(message.id, { resolve, reject });
    });
  }

  _handleMessage(message) {
    if (message.type === "ok") {
      this.pendingResponses.get(message.id).resolve();
      this.pendingResponses.delete(message.id);
    } else if (message.type === "error") {
      this.pendingResponses.get(message.id).reject(message.description);
      this.pendingResponses.delete(message.id);
    } else {
      const watchId = message.watchId;
      delete message.watchId;
      const callback = this.watchCallbacks.get(watchId);
      if (callback) {
        callback(message);
      }
    }
  }
};
