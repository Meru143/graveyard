const path = require("node:path");
const { execFileSync } = require("node:child_process");

const repoRoot = path.resolve(__dirname, "..");

execFileSync("node", [path.join(repoRoot, "scripts", "prepare-npm-packages.js"), "cleanup"], {
  stdio: "inherit"
});
