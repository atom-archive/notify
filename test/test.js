const assert = require("assert");
const path = require("path");
const fs = require("fs");
const temp = require("temp");
const Watcher = require("..");

temp.track();

describe("watchPath", () => {
  let tempDirPath;
  let watcher;

  beforeEach(() => {
    tempDirPath = fs
      .realpathSync(temp.mkdirSync())
      .replace("VSSADM~1", "VssAdministrator"); // Hack to fix Azure DevOps Windows builds 🙄
    watcher = new Watcher();
  });

  afterEach(() => {
    watcher.kill();
    watcher = null;
  });

  it("tracks events in watched directories", async () => {
    const events = [];
    await watcher.watchPath(tempDirPath, event => events.push(event));

    fs.writeFileSync(path.join(tempDirPath, "foo"), "");

    await condition(() => events.length === 1);

    assert.deepStrictEqual(events, [
      {
        action: "created",
        path: path.join(tempDirPath, "foo")
      }
    ]);
  });

  it("rejects when watching a path that does not exist", async () => {
    await assert.rejects(
      () =>
        watcher.watchPath(
          path.join(tempDirPath, "does-not-exist"),
          () => {}
        ),
      "No path was found"
    );
  });
});

function condition(fn) {
  return new Promise((resolve, reject) => {
    const timeoutError = new Error("Condition timed out");
    Error.captureStackTrace(timeoutError, condition);

    const interval = global.setInterval(() => {
      if (fn()) {
        global.clearTimeout(timeout);
        global.clearInterval(interval);
        resolve();
      }
    }, 10);

    const timeout = global.setTimeout(() => {
      global.clearInterval(interval);
      reject(timeoutError);
    }, 1000);
  });
}
