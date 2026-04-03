#!/usr/bin/env node

const { existsSync } = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const PLATFORM_PACKAGES = {
  "darwin:arm64": "graveyard-darwin-arm64",
  "darwin:x64": "graveyard-darwin-x64",
  "linux:arm64": "graveyard-linux-arm64",
  "linux:x64": "graveyard-linux-x64",
  "win32:x64": "graveyard-windows-x64"
};

const binaryName = process.platform === "win32" ? "graveyard.exe" : "graveyard";
const platformKey = `${process.platform}:${process.arch}`;
const fallbackBinary = path.join(__dirname, binaryName);

function resolveOptionalBinary() {
  const packageName = PLATFORM_PACKAGES[platformKey];
  if (!packageName) {
    return null;
  }

  try {
    const packageJsonPath = require.resolve(`${packageName}/package.json`);
    const candidate = path.join(path.dirname(packageJsonPath), "bin", binaryName);
    return existsSync(candidate) ? candidate : null;
  } catch {
    return null;
  }
}

const binaryPath = resolveOptionalBinary() ?? (existsSync(fallbackBinary) ? fallbackBinary : null);

if (!binaryPath) {
  console.error(
    `[ERROR] No npm binary is available for ${process.platform}/${process.arch}.`
  );
  process.exit(1);
}

const result = spawnSync(binaryPath, process.argv.slice(2), {
  stdio: "inherit",
  windowsHide: false
});

if (result.error) {
  console.error(`[ERROR] Failed to execute ${binaryPath}: ${result.error.message}`);
  process.exit(1);
}

if (result.status !== null) {
  process.exit(result.status);
}

process.exit(1);
