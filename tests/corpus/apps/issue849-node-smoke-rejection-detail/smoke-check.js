const fs = require("fs");
const path = require("path");
const assert = require("assert");

const manifest = fs.readFileSync(path.join(__dirname, "package.json"), "utf8");

assert.ok(
  /"name"\s*:\s*[{"]/.test(manifest),
  `package.json must name the app: ${manifest.length}`,
);
