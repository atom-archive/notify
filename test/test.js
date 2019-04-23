const assert = require("assert");
const fs = require("fs");
const temp = require("temp");
const Supervisor = require("..");

temp.track();

describe("watchPath", () => {
  let tempDirPath;
  let supervisor;

  beforeEach(() => {
    tempDirPath = fs.realpathSync(temp.mkdirSync());
    supervisor = new Supervisor();
  });

  afterEach(() => {
    supervisor.kill();
    supervisor = null;
  });

  it("tracks events in watched directories", async () => {
    const events = [];
    await supervisor.watchPath(`${tempDirPath}`, event => events.push(event));

    fs.writeFileSync(`${tempDirPath}/foo`, "");

    await condition(() => events.length === 1);

    assert.deepStrictEqual(events, [
      {
        action: "created",
        path: `${tempDirPath}/foo`
      }
    ]);
  });

  it("rejects when watching a path that does not exist", async () => {
    await assert.rejects(
      () => supervisor.watchPath(`${tempDirPath}/does/not/exist`, () => {}),
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
