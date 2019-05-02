const { spawn } = require("child_process");

module.exports = class Watcher {
  constructor() {
    this.nextId = 0;
    this.watchCallbacks = new Map();
    this.pendingResponses = new Map();
    this.startPromise = new Promise((resolve, reject) => {
      this.childProcess = spawn(
        `${__dirname}/../notify-subprocess-${process.platform}`,
        {
          stdio: ["pipe", "pipe", "pipe"]
        }
      );

      let started = false;
      let stderr = "";
      this.childProcess.stderr.on("data", data => (stderr += data));
      this.childProcess.on("exit", code => {
        console.error("CRASH!", started)
        this.subprocessCrash = `Notify subprocess exited with code ${code}:\n\n${stderr}`;
        if (code && !started) {
          console.error("REJECT")
          reject(new Error(this.subprocessCrash));
        }
      });

      let buffer = "";
      this.childProcess.stdout.on("data", chunk => {
        buffer += chunk;
        const lines = buffer.split("\n");
        buffer = lines.pop();
        for (const line of lines) {
          if (!started && line === "Listening") {
            console.error("STARTED!")
            started = true;
            resolve();
          } else {
            this._handleMessage(JSON.parse(line));
          }
        }
      });
    });
  }

  kill() {
    if (this.childProcess) {
      this.childProcess.stdin.end();
      this.childProcess.kill();
      this.childProcess = null;
    }
  }

  async watchPath(path, callback) {
    console.error("await")
    await this.startPromise;
    console.error("done")

    if (this.subprocessCrash) {
      throw new Error(this.subprocessCrash);
    }

    if (!this.childProcess) {
      throw new Error("This watcher has been killed");
    }

    let id = this.nextId++;
    this.watchCallbacks.set(id, callback);
    await this._sendRequest({
      type: "watch",
      id: id,
      root: path
    });

    let disposed = false;
    return {
      dispose: async () => {
        if (!disposed) {
          disposed = true;
          await this._unwatch(id);
        }
      }
    };
  }

  async _unwatch(id) {
    if (this.childProcess) {
      this.watchCallbacks.delete(id);
      await this._sendRequest({
        type: "unwatch",
        id: id
      });
    }
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
      const callback = this.watchCallbacks.get(message.watchId);
      if (callback) {
        callback(message.events);
      }
    }
  }
};
