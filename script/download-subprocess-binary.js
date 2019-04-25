const fs = require("fs");
const https = require("https");
const path = require("path");

const packageVersion = process.env.npm_package_version;

let artifactName = `notify-subprocess-${process.platform}`;
if (process.platform === "win32") artifactName += ".exe";
const artifactUrl = `https://github.com/atom/notify/releases/download/v${packageVersion}/${artifactName}`;

const binaryPath = path.join(__dirname, "..", artifactName);

console.log(
  "Downloading notify subprocess binary from GitHub release:",
  artifactUrl
);
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
        response.pipe(fs.createWriteStream(binaryPath));
        return;
      }
      default: {
        console.error("Error downloading binary:", response.headers.status);
        return;
      }
    }
  }
}
