import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { execFileSync } from "node:child_process";
import { fileURLToPath, pathToFileURL } from "node:url";

const rootFiles = new Set([
  ".gitignore",
  ".gitattributes",
  ".nvmrc",
  "Cargo.toml",
  "Cargo.lock",
  "LICENSE",
  "THIRD_PARTY_NOTICES.md",
  "README.md",
  "feature_checklist.md",
  "package.json",
  "package-lock.json",
  "rust-toolchain.toml",
  "index.html",
  "tsconfig.json",
  "vite.config.ts",
]);
const documents = new Set([
  "design.md",
  "implementation-plan.md",
  "compatibility.md",
  "validation.md",
  "ui-shell.md",
  "formats.md",
  "writing.md",
  "search.md",
  "opening-tree.md",
  "game-list.md",
  "game-window.md",
  "engines.md",
]);
export function allowedSourcePath(candidate) {
  if (
    typeof candidate !== "string" ||
    !candidate ||
    /[\\\0:]/.test(candidate) ||
    candidate.startsWith("/")
  )
    return false;
  if (
    candidate.split("/").some((part) => !part || part === "." || part === "..")
  )
    return false;
  if (
    /\.(?:cb[a-z0-9]*|2cb[a-z0-9]*|cib2?|cit2?|cpo|cko|2l[a-z0-9]*|exe|dll|msi|zip|dmg|app|bin)$/i.test(
      candidate,
    )
  )
    return false;
  if (rootFiles.has(candidate)) return true;
  if (/^crates\/(?:chess-core|cbh-reader|engine)\/Cargo\.toml$/.test(candidate))
    return true;
  if (
    /^crates\/(?:chess-core|cbh-reader|engine)\/(?:src|tests|examples)\/[a-zA-Z0-9_/-]+\.rs$/.test(
      candidate,
    )
  )
    return true;
  if (/^src\/[a-zA-Z0-9_./-]+\.(?:ts|tsx|css)$/.test(candidate)) return true;
  if (/^src-tauri\/(?:src|tests|examples)\/[a-zA-Z0-9_/-]+\.rs$/.test(candidate))
    return true;
  if (
    [
      "src-tauri/Cargo.toml",
      "src-tauri/build.rs",
      "src-tauri/tauri.conf.json",
      "src-tauri/capabilities/main.json",
      "src-tauri/icons/icon.png",
    ].includes(candidate)
  )
    return true;
  if (candidate.startsWith("docs/") && documents.has(candidate.slice(5)))
    return true;
  if (
    [
      "scripts/check-distribution.mjs",
      "scripts/generate-notices.mjs",
      "scripts/compare-corpus.py",
      "tests/fixtures/README.md",
      "tests/fixtures/synthetic-start.pgn",
      "licenses/cbh2pgn-MIT.txt",
      "licenses/dependencies.json",
      ".github/workflows/desktop.yml",
    ].includes(candidate)
  )
    return true;
  return (
    /^tests\/[a-zA-Z0-9_-]+\.test\.ts$/.test(candidate) ||
    /^licenses\/notices\/[a-f0-9]{64}\.txt$/.test(candidate)
  );
}
export function auditCandidatePaths(paths) {
  return paths.filter((candidate) => !allowedSourcePath(candidate));
}
export const digest = (bytes) =>
  crypto.createHash("sha256").update(bytes).digest("hex");

export function checkDistribution(root) {
  const tracked = execFileSync("git", ["ls-files", "-z"], {
    cwd: root,
    encoding: "utf8",
  })
    .split("\0")
    .filter(Boolean);
  const errors = auditCandidatePaths(tracked).map(
    (name) => `Unapproved source path: ${name}`,
  );
  const trackedSet = new Set(tracked);
  for (const name of tracked) {
    const file = path.join(root, name);
    if (!fs.existsSync(file) || !fs.lstatSync(file).isFile()) {
      errors.push(`Source must be a regular file: ${name}`);
      continue;
    }
    if (!name.startsWith("licenses/") && !name.endsWith(".png")) {
      const content = fs.readFileSync(file, "utf8");
      if (
        /(?:\/Users\/|\/home\/)[\w.-]+\/|[A-Z]:\\Users\\[^\\]+\\/.test(content)
      )
        errors.push(`Private home path in ${name}`);
    }
  }
  const inventoryPath = path.join(root, "licenses/dependencies.json");
  if (!trackedSet.has("licenses/dependencies.json"))
    errors.push("Missing tracked dependency inventory");
  if (fs.existsSync(inventoryPath)) {
    const inventory = JSON.parse(fs.readFileSync(inventoryPath, "utf8"));
    for (const lock of ["Cargo.lock", "package-lock.json"]) {
      if (
        inventory.locks[lock] !== digest(fs.readFileSync(path.join(root, lock)))
      )
        errors.push(`Dependency notices are stale for ${lock}`);
    }
    const cargoLock = fs.readFileSync(path.join(root, "Cargo.lock"), "utf8");
    const rust = cargoLock
      .split("[[package]]")
      .slice(1)
      .filter((block) => /^source = /m.test(block))
      .map(
        (block) =>
          `${block.match(/^name = "([^"]+)"/m)[1]}@${block.match(/^version = "([^"]+)"/m)[1]}`,
      )
      .sort();
    const npmLock = JSON.parse(
      fs.readFileSync(path.join(root, "package-lock.json"), "utf8"),
    );
    const npm = Object.entries(npmLock.packages)
      .filter(([name, data]) => name && !data.dev)
      .map(([name, data]) => `${name}@${data.version}`)
      .sort();
    for (const [ecosystem, expected] of [
      ["rust", rust],
      ["npm", npm],
    ]) {
      const actual = inventory[ecosystem]
        .map((pkg) => `${pkg.name}@${pkg.version}`)
        .sort();
      if (JSON.stringify(expected) !== JSON.stringify(actual))
        errors.push(`Incomplete ${ecosystem} dependency inventory`);
    }
    const referenced = new Set();
    for (const pkg of [...inventory.rust, ...inventory.npm]) {
      if (!pkg.license || !pkg.notices.length)
        errors.push(`Missing license/notice: ${pkg.name}@${pkg.version}`);
      for (const notice of pkg.notices) {
        if (!/^licenses\/notices\/[a-f0-9]{64}\.txt$/.test(notice.path)) {
          errors.push(`Invalid notice path for ${pkg.name}`);
          continue;
        }
        referenced.add(notice.path);
        if (
          !trackedSet.has(notice.path) ||
          !fs.existsSync(path.join(root, notice.path))
        )
          errors.push(`Missing tracked notice: ${notice.path}`);
        else if (
          digest(fs.readFileSync(path.join(root, notice.path))) !==
          notice.sha256
        )
          errors.push(`Changed notice: ${notice.path}`);
      }
    }
    for (const name of tracked.filter((name) =>
      name.startsWith("licenses/notices/"),
    )) {
      if (!referenced.has(name))
        errors.push(`Unreferenced notice file: ${name}`);
    }
  }
  return { files: tracked.length, errors };
}
if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href
) {
  const result = checkDistribution(
    path.resolve(path.dirname(fileURLToPath(import.meta.url)), ".."),
  );
  if (result.errors.length) {
    console.error(result.errors.join("\n"));
    process.exitCode = 1;
  } else
    console.log(
      `Source boundary and locked dependency notices verified (${result.files} tracked files).`,
    );
}
