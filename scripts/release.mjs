#!/usr/bin/env node
import { spawnSync, execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  statSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const packageJsonPath = path.join(repoRoot, "package.json");
const tauriConfigPath = path.join(repoRoot, "src-tauri", "tauri.conf.json");
const cargoTomlPath = path.join(repoRoot, "src-tauri", "Cargo.toml");
const cargoLockPath = path.join(repoRoot, "src-tauri", "Cargo.lock");
const releaseArtifactsRoot = path.join(repoRoot, "target", "release-artifacts");
const semverPattern = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/;
const binaryNames = ["agenta", "agenta-cli", "agenta-mcp", "agenta-desktop"];
const cargoCliBuildArgs = [
  "build",
  "--release",
  "--manifest-path",
  "src-tauri/Cargo.toml",
  "--bin",
  "agenta",
  "--bin",
  "agenta-cli",
  "--bin",
  "agenta-mcp",
];

main();

function main() {
  try {
    const options = parseArgs(process.argv.slice(2));
    if (options.help) {
      printHelp();
      return;
    }

    if (options.setVersion) {
      validateVersion(options.setVersion);
      setVersionSources(options.setVersion, options.dryRun);
      if (!options.dryRun) {
        console.log(`Updated version sources to ${options.setVersion}.`);
        console.log("Commit the version bump, then run `bun run release`.");
        return;
      }
    }

    const versions = readVersions(options.setVersion);
    validateVersion(versions.packageVersion);
    assertVersionsMatch(versions);

    const git = readGitMetadata();
    if (git.git_dirty && !options.allowDirty && !options.dryRun) {
      throw new Error("Worktree is dirty. Commit changes or pass --allow-dirty for a non-official build.");
    }

    const targetTriple = readTargetTriple();
    const artifactVersion = formatArtifactVersion(versions.packageVersion, git);
    const releaseDirName = `agenta-v${artifactVersion}`;
    const releaseDir = path.join(releaseArtifactsRoot, releaseDirName);
    const buildEnv = {
      AGENTA_BUILD_FORCE_RERUN: new Date().toISOString(),
    };

    const packageBuildCommand = resolvePackageBuildCommand();

    printPlan({
      artifactVersion,
      buildCommands: [
        `cargo ${cargoCliBuildArgs.join(" ")}`,
        packageBuildCommand.label,
      ],
      git,
      options,
      releaseDir,
      targetTriple,
      version: versions.packageVersion,
    });

    if (options.dryRun) {
      return;
    }

    if (existsSync(releaseDir)) {
      throw new Error(`Release directory already exists: ${releaseDir}`);
    }

    run("cargo", cargoCliBuildArgs, buildEnv);
    run(packageBuildCommand.command, packageBuildCommand.args, buildEnv);

    mkdirSync(path.join(releaseDir, "installers"), { recursive: true });
    mkdirSync(path.join(releaseDir, "bin"), { recursive: true });

    const artifacts = [
      ...copyInstallers(releaseDir),
      ...copyBinaries(releaseDir, artifactVersion, targetTriple),
    ];

    const manifestPath = path.join(releaseDir, "manifest.json");
    const manifest = {
      product: "Agenta",
      version: versions.packageVersion,
      display_version: artifactVersion,
      git_commit: git.git_commit,
      git_commit_short: git.git_commit_short,
      git_describe: git.git_describe,
      git_dirty: git.git_dirty,
      target_triple: targetTriple,
      built_at: new Date().toISOString(),
      artifacts,
    };
    writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);

    writeChecksums(releaseDir, [...artifacts, artifactRecord(releaseDir, manifestPath, "manifest")]);

    console.log(`Release artifacts written to ${releaseDir}`);
  } catch (error) {
    console.error(error instanceof Error ? error.message : error);
    process.exitCode = 1;
  }
}

function parseArgs(args) {
  const options = {
    allowDirty: false,
    dryRun: false,
    help: false,
    setVersion: null,
  };

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--allow-dirty") {
      options.allowDirty = true;
    } else if (arg === "--dry-run") {
      options.dryRun = true;
    } else if (arg === "--help" || arg === "-h") {
      options.help = true;
    } else if (arg === "--set-version") {
      const version = args[index + 1];
      if (!version) {
        throw new Error("--set-version requires a version value.");
      }
      options.setVersion = version;
      index += 1;
    } else if (arg.startsWith("--set-version=")) {
      options.setVersion = arg.slice("--set-version=".length);
    } else {
      throw new Error(`Unknown release option: ${arg}`);
    }
  }

  return options;
}

function printHelp() {
  console.log(`Usage: node scripts/release.mjs [options]

Options:
  --dry-run              Print version metadata and commands without building.
  --allow-dirty          Allow a non-official release from a dirty worktree.
  --set-version <ver>    Update package.json, tauri.conf.json, Cargo.toml, and Cargo.lock.
  -h, --help             Show this help.
`);
}

function readVersions(overrideVersion = null) {
  return {
    packageVersion: overrideVersion ?? readJson(packageJsonPath).version,
    tauriVersion: overrideVersion ?? readJson(tauriConfigPath).version,
    cargoVersion: overrideVersion ?? readCargoPackageVersion(),
  };
}

function setVersionSources(version, dryRun) {
  if (dryRun) {
    console.log(`[dry-run] Would update version sources to ${version}.`);
    return;
  }

  const packageJson = readJson(packageJsonPath);
  packageJson.version = version;
  writeJson(packageJsonPath, packageJson);

  const tauriConfig = readJson(tauriConfigPath);
  tauriConfig.version = version;
  writeJson(tauriConfigPath, tauriConfig);

  const cargoToml = readFileSync(cargoTomlPath, "utf8");
  const updatedCargoToml = cargoToml.replace(
    /(^\[package\][\s\S]*?^version\s*=\s*)"[^"]+"/m,
    (_match, prefix) => `${prefix}"${version}"`,
  );
  if (updatedCargoToml === cargoToml) {
    throw new Error("Could not find [package] version in src-tauri/Cargo.toml.");
  }
  writeFileSync(cargoTomlPath, updatedCargoToml);

  if (existsSync(cargoLockPath)) {
    const cargoLock = readFileSync(cargoLockPath, "utf8");
    const updatedCargoLock = cargoLock.replace(
      /(\[\[package\]\]\r?\nname = "agenta"\r?\nversion = ")[^"]+"/,
      (_match, prefix) => `${prefix}${version}"`,
    );
    if (updatedCargoLock !== cargoLock) {
      writeFileSync(cargoLockPath, updatedCargoLock);
    }
  }
}

function readJson(filePath) {
  return JSON.parse(readFileSync(filePath, "utf8"));
}

function writeJson(filePath, value) {
  writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`);
}

function readCargoPackageVersion() {
  const cargoToml = readFileSync(cargoTomlPath, "utf8");
  const match = cargoToml.match(/^\[package\][\s\S]*?^version\s*=\s*"([^"]+)"/m);
  if (!match) {
    throw new Error("Could not find [package] version in src-tauri/Cargo.toml.");
  }
  return match[1];
}

function validateVersion(version) {
  if (!semverPattern.test(version)) {
    throw new Error(`Version must be semver without build metadata: ${version}`);
  }
}

function assertVersionsMatch(versions) {
  const unique = new Set(Object.values(versions));
  if (unique.size !== 1) {
    throw new Error(
      `Version sources do not match: package=${versions.packageVersion}, tauri=${versions.tauriVersion}, cargo=${versions.cargoVersion}`,
    );
  }
}

function readGitMetadata() {
  const git_commit = git(["rev-parse", "HEAD"]);
  const git_commit_short = git(["rev-parse", "--short", "HEAD"]);
  const git_describe = git(["describe", "--tags", "--always", "--dirty"]);
  const status = git(["status", "--porcelain"]);

  return {
    git_commit,
    git_commit_short,
    git_describe,
    git_dirty: status === null ? false : status.length > 0,
  };
}

function git(args) {
  try {
    return execFileSync("git", args, {
      cwd: repoRoot,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    }).trim();
  } catch {
    return null;
  }
}

function readTargetTriple() {
  try {
    const output = execFileSync("rustc", ["-vV"], {
      cwd: repoRoot,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    });
    const hostLine = output
      .split(/\r?\n/)
      .find((line) => line.startsWith("host: "));
    return hostLine ? hostLine.slice("host: ".length).trim() : "unknown-target";
  } catch {
    return "unknown-target";
  }
}

function formatArtifactVersion(version, git) {
  if (!git.git_commit_short) {
    return `${version}+nogit`;
  }
  return `${version}+${git.git_commit_short}${git.git_dirty ? ".dirty" : ""}`;
}

function resolvePackageBuildCommand() {
  const userAgent = process.env.npm_config_user_agent ?? "";
  const npmExecPath = process.env.npm_execpath;

  if (userAgent.startsWith("bun/") && npmExecPath) {
    return {
      args: ["run", "tauri", "build"],
      command: npmExecPath,
      label: "bun run tauri build",
    };
  }

  if (userAgent.startsWith("npm/")) {
    return {
      args: ["run", "tauri", "build"],
      command: process.platform === "win32" ? "npm.cmd" : "npm",
      label: "npm run tauri build",
    };
  }

  return {
    args: ["run", "tauri", "build"],
    command: "bun",
    label: "bun run tauri build",
  };
}

function printPlan({ artifactVersion, buildCommands, git, options, releaseDir, targetTriple, version }) {
  console.log(`Version: ${version}`);
  console.log(`Display version: ${artifactVersion}`);
  console.log(`Git commit: ${git.git_commit ?? "N/A"}`);
  console.log(`Git describe: ${git.git_describe ?? "N/A"}`);
  console.log(`Git dirty: ${git.git_dirty ? "true" : "false"}`);
  console.log(`Target: ${targetTriple}`);
  console.log(`Release dir: ${releaseDir}`);

  if (git.git_dirty && !options.allowDirty && options.dryRun) {
    console.log("[dry-run] Official release would fail because the worktree is dirty.");
  }

  for (const command of buildCommands) {
    console.log(`[${options.dryRun ? "dry-run" : "run"}] ${command}`);
  }
}

function run(command, args, extraEnv = {}) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    env: {
      ...process.env,
      ...extraEnv,
    },
    shell: false,
    stdio: "inherit",
  });

  if (result.error) {
    throw result.error;
  }

  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed with exit code ${result.status}.`);
  }
}

function copyInstallers(releaseDir) {
  const bundleDir = path.join(repoRoot, "src-tauri", "target", "release", "bundle");
  if (!existsSync(bundleDir)) {
    throw new Error(`Missing Tauri bundle output: ${bundleDir}`);
  }

  const artifacts = [];
  for (const sourcePath of listFiles(bundleDir)) {
    const relativeSource = path.relative(bundleDir, sourcePath);
    const targetPath = path.join(releaseDir, "installers", relativeSource);
    mkdirSync(path.dirname(targetPath), { recursive: true });
    copyFileSync(sourcePath, targetPath);
    artifacts.push(artifactRecord(releaseDir, targetPath, "installer"));
  }

  if (artifacts.length === 0) {
    throw new Error(`No installer artifacts found in ${bundleDir}`);
  }

  return artifacts;
}

function copyBinaries(releaseDir, artifactVersion, targetTriple) {
  const sourceDir = path.join(repoRoot, "src-tauri", "target", "release");
  const artifacts = [];

  for (const binaryName of binaryNames) {
    const sourcePath = resolveBinaryPath(sourceDir, binaryName);
    const extension = path.extname(sourcePath);
    const originalTarget = path.join(releaseDir, "bin", path.basename(sourcePath));
    const versionedTarget = path.join(
      releaseDir,
      "bin",
      `${binaryName}-v${artifactVersion}-${targetTriple}${extension}`,
    );

    copyFileSync(sourcePath, originalTarget);
    copyFileSync(sourcePath, versionedTarget);
    artifacts.push(artifactRecord(releaseDir, originalTarget, "binary"));
    artifacts.push(artifactRecord(releaseDir, versionedTarget, "binary"));
  }

  return artifacts;
}

function resolveBinaryPath(sourceDir, binaryName) {
  const executableExtension = process.platform === "win32" ? ".exe" : "";
  const candidates = [
    path.join(sourceDir, `${binaryName}${executableExtension}`),
    path.join(sourceDir, binaryName),
    path.join(sourceDir, `${binaryName}.exe`),
  ];
  const sourcePath = candidates.find((candidate) => existsSync(candidate));
  if (!sourcePath) {
    throw new Error(`Missing release binary for ${binaryName} in ${sourceDir}`);
  }
  return sourcePath;
}

function listFiles(directory) {
  const entries = readdirSync(directory, { withFileTypes: true });
  return entries.flatMap((entry) => {
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      return listFiles(entryPath);
    }
    return entry.isFile() ? [entryPath] : [];
  });
}

function artifactRecord(releaseDir, filePath, kind) {
  return {
    kind,
    name: path.basename(filePath),
    path: slash(path.relative(releaseDir, filePath)),
    sha256: sha256(filePath),
    size_bytes: statSync(filePath).size,
  };
}

function writeChecksums(releaseDir, artifacts) {
  const lines = artifacts
    .map((artifact) => `${artifact.sha256}  ${artifact.path}`)
    .sort((left, right) => left.localeCompare(right));
  writeFileSync(path.join(releaseDir, "SHA256SUMS"), `${lines.join("\n")}\n`);
}

function sha256(filePath) {
  return createHash("sha256").update(readFileSync(filePath)).digest("hex");
}

function slash(value) {
  return value.split(path.sep).join("/");
}
