const { execSync } = require("child_process");
const fs = require("fs");
const path = require("path");

const release =
  process.argv.includes("-r") || process.argv.includes("--release")
    ? "--release"
    : "";

const buildCommand = `cargo build ${release}`;
console.log("Building subprocess binary locally:", buildCommand);
execSync(buildCommand, {
  cwd: path.join(__dirname, ".."),
  stdio: "inherit"
});

let srcPath = path.join(
  __dirname,
  "..",
  "target",
  "debug",
  "notify-subprocess"
);
if (process.platform === "win32") srcPath += ".exe";

let dstPath = path.join(
  __dirname,
  "..",
  `notify-subprocess-${process.platform}`
);
if (process.platform === "win32") dstPath += ".exe";

console.log(
  "Overwriting downloaded subprocess binary with locally-built version for development"
);
fs.copyFileSync(srcPath, dstPath);
