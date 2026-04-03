const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { execFileSync } = require("node:child_process");

const repoRoot = path.resolve(__dirname, "..");
const mode = process.argv[2] ?? "local";
const args = process.argv.slice(3);

const PACKAGES = [
  {
    workspace: "graveyard-darwin-arm64",
    archiveSuffix: "_darwin_arm64.tar.gz",
    binaryName: "graveyard"
  },
  {
    workspace: "graveyard-darwin-x64",
    archiveSuffix: "_darwin_amd64.tar.gz",
    binaryName: "graveyard"
  },
  {
    workspace: "graveyard-linux-arm64",
    archiveSuffix: "_linux_arm64.tar.gz",
    binaryName: "graveyard"
  },
  {
    workspace: "graveyard-linux-x64",
    archiveSuffix: "_linux_amd64.tar.gz",
    binaryName: "graveyard"
  },
  {
    workspace: "graveyard-windows-x64",
    archiveSuffix: "_windows_amd64.zip",
    binaryName: "graveyard.exe"
  }
];

const CURRENT_WORKSPACE = {
  "darwin:arm64": "graveyard-darwin-arm64",
  "darwin:x64": "graveyard-darwin-x64",
  "linux:arm64": "graveyard-linux-arm64",
  "linux:x64": "graveyard-linux-x64",
  "win32:x64": "graveyard-windows-x64"
}[`${process.platform}:${process.arch}`];

function mkdirp(dirPath) {
  fs.mkdirSync(dirPath, { recursive: true });
}

function copyBinary(sourcePath, destinationPath) {
  mkdirp(path.dirname(destinationPath));
  fs.copyFileSync(sourcePath, destinationPath);
  if (path.extname(destinationPath) === "") {
    fs.chmodSync(destinationPath, 0o755);
  }
}

function removeIfExists(filePath) {
  if (fs.existsSync(filePath)) {
    fs.rmSync(filePath, { force: true });
  }
}

function findArg(flag, fallbackValue) {
  const index = args.indexOf(flag);
  if (index === -1 || index + 1 >= args.length) {
    return fallbackValue;
  }

  return args[index + 1];
}

function extractArchive(archivePath, destinationDir) {
  mkdirp(destinationDir);
  const script = [
    "import pathlib, shutil, sys, tarfile, zipfile",
    "archive = pathlib.Path(sys.argv[1])",
    "dest = pathlib.Path(sys.argv[2])",
    "shutil.rmtree(dest, ignore_errors=True)",
    "dest.mkdir(parents=True, exist_ok=True)",
    "if archive.name.endswith('.zip'):",
    "    with zipfile.ZipFile(archive) as zf:",
    "        zf.extractall(dest)",
    "else:",
    "    with tarfile.open(archive) as tf:",
    "        tf.extractall(dest)"
  ].join("\n");

  execFileSync("python", ["-c", script, archivePath, destinationDir], {
    stdio: "inherit"
  });
}

function findBinary(rootDir, binaryName) {
  const queue = [rootDir];
  while (queue.length > 0) {
    const current = queue.pop();
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const fullPath = path.join(current, entry.name);
      if (entry.isDirectory()) {
        queue.push(fullPath);
      } else if (entry.isFile() && entry.name === binaryName) {
        return fullPath;
      }
    }
  }

  throw new Error(`binary ${binaryName} was not found in ${rootDir}`);
}

function prepareLocal() {
  const binaryName = process.platform === "win32" ? "graveyard.exe" : "graveyard";
  const sourceBinary = path.join(repoRoot, "target", "release", binaryName);

  if (!fs.existsSync(sourceBinary)) {
    throw new Error(`expected release binary at ${sourceBinary}`);
  }

  copyBinary(sourceBinary, path.join(repoRoot, "bin", binaryName));

  if (CURRENT_WORKSPACE) {
    copyBinary(
      sourceBinary,
      path.join(repoRoot, "npm", CURRENT_WORKSPACE, "bin", binaryName)
    );
  }
}

function prepareFromDist() {
  const distDir = path.resolve(repoRoot, findArg("--dist-dir", "dist"));
  if (!fs.existsSync(distDir)) {
    throw new Error(`dist directory not found: ${distDir}`);
  }

  const distEntries = fs.readdirSync(distDir);
  const extractRoot = path.join(repoRoot, ".npm-dist");
  fs.rmSync(extractRoot, { force: true, recursive: true });
  mkdirp(extractRoot);

  for (const pkg of PACKAGES) {
    const archiveName = distEntries.find((entry) => entry.endsWith(pkg.archiveSuffix));
    if (!archiveName) {
      throw new Error(`archive ending with ${pkg.archiveSuffix} not found in ${distDir}`);
    }

    const archivePath = path.join(distDir, archiveName);
    const extractDir = path.join(extractRoot, pkg.workspace);
    extractArchive(archivePath, extractDir);

    const sourceBinary = findBinary(extractDir, pkg.binaryName);
    const destinationBinary = path.join(
      repoRoot,
      "npm",
      pkg.workspace,
      "bin",
      pkg.binaryName
    );
    copyBinary(sourceBinary, destinationBinary);
  }
}

function cleanup() {
  removeIfExists(path.join(repoRoot, "bin", "graveyard"));
  removeIfExists(path.join(repoRoot, "bin", "graveyard.exe"));
  removeIfExists(path.join(repoRoot, ".npm-dist"));
  for (const pkg of PACKAGES) {
    removeIfExists(path.join(repoRoot, "npm", pkg.workspace, "bin", pkg.binaryName));
  }
}

if (mode === "dist") {
  prepareFromDist();
} else if (mode === "cleanup") {
  cleanup();
} else {
  prepareLocal();
}
