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

let binDirPath = path.join(__dirname, "..", "bin");
let dstPath = path.join(binDirPath, `notify-subprocess-${process.platform}`);
if (process.platform === "win32") dstPath += ".exe";

console.log(
  "Overwriting downloaded subprocess binary with locally-built version for development"
);
if (!fs.existsSync(binDirPath)) fs.mkdirSync(binDirPath)
fs.copyFileSync(srcPath, dstPath);
