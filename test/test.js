const assert = require("assert");
const path = require("path");
const fs = require("fs");
const temp = require("temp");
const Supervisor = require("..");

temp.track();

describe("watchPath", () => {
  let tempDirPath;
  let supervisor;

  beforeEach(() => {
    tempDirPath = fs
      .realpathSync(temp.mkdirSync())
      .replace("VSSADM~1", "VssAdministrator"); // Hack to fix Azure DevOps Windows builds 🙄
    supervisor = new Supervisor({ mode: "debug" });
  });

  afterEach(() => {
    supervisor.kill();
    supervisor = null;
  });

  it("tracks events in watched directories", async () => {
    const events = [];
    await supervisor.watchPath(tempDirPath, event => events.push(event));

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
        supervisor.watchPath(
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
