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

  it("tracks events in watched directories until subscriptions are disposed", async () => {
    const dirEvents = [];
    const subscription = await watcher.watchPath(tempDirPath, events =>
      dirEvents.push(...events)
    );

    fs.writeFileSync(path.join(tempDirPath, "a"), "");
    fs.mkdirSync(path.join(tempDirPath, "subdir"));

    await condition(() => dirEvents.length === 2);

    assert.deepStrictEqual(dirEvents, [
      {
        action: "created",
        path: path.join(tempDirPath, "a")
      },
      {
        action: "created",
        path: path.join(tempDirPath, "subdir")
      }
    ]);

    // Watch subdir and dispose of subscription on parent dir
    const subdirEvents = [];
    await watcher.watchPath(path.join(tempDirPath, "subdir"), events =>
      subdirEvents.push(...events)
    );
    dirEvents.length = 0;
    await subscription.dispose();

    fs.writeFileSync(path.join(tempDirPath, "subdir", "b"), "");

    await condition(() => subdirEvents.length === 1);

    // Event observed via watch on subdir, but not on parent dir
    assert.strictEqual(dirEvents.length, 0);
  });

  it("rejects when watching a path that does not exist", async () => {
    await assert.rejects(
      () =>
        watcher.watchPath(path.join(tempDirPath, "does-not-exist"), () => {}),
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
