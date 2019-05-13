const fs = require("fs");
const https = require("https");
const path = require("path");

const packageVersion = process.env.npm_package_version;

let artifactName = `notify-subprocess-${process.platform}`;
if (process.platform === "win32") artifactName += ".exe";
const artifactUrl = `https://github.com/atom/notify/releases/download/v${packageVersion}/${artifactName}`;

const binDirPath = path.join(__dirname, "..", "bin");
const binPath = path.join(binDirPath, artifactName);

console.log(
  "Downloading notify subprocess binary from GitHub release:",
  artifactUrl
);
if (!fs.existsSync(binDirPath)) fs.mkdirSync(binDirPath);
downloadBinary(artifactUrl);

async function downloadBinary(url) {
  while (true) {
    const response = await new Promise(resolve => https.get(url, resolve));
    switch (response.statusCode) {
      case 302: {
        url = response.headers.location;
        break;
      }
      case 200: {
        fs.writeFileSync(binPath, "");
        fs.chmodSync(binPath, 0o755);
        response.pipe(fs.createWriteStream(binPath));
        return;
      }
      default: {
        console.error("Error downloading binary:", response.headers.status);
        return;
      }
    }
  }
}
