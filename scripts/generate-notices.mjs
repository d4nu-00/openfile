// Regenerate after dependency updates. Network access is opt-in, for pinned
// upstream license files omitted from published crates; no source code is copied.
import fs from "node:fs";
import path from "node:path";
import { digest } from "./check-distribution.mjs";

const args = process.argv.slice(2);
const value = (flag) => args[args.indexOf(flag) + 1];
if (!args.includes("--metadata") || !args.includes("--cache")) {
  throw new Error(
    "Usage: node scripts/generate-notices.mjs --metadata <cargo-metadata.json> --cache <external-cache-dir> [--fetch]",
  );
}
const metadata = JSON.parse(fs.readFileSync(value("--metadata"), "utf8"));
const cache = path.resolve(value("--cache"));
fs.mkdirSync(cache, { recursive: true });
fs.mkdirSync("licenses/notices", { recursive: true });
const licenseName = (name) =>
  /^(?:licen[cs]es?|copying|notice|copyright)(?:[._-]|$)/i.test(name);
function localNotices(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    if (!licenseName(entry.name)) return [];
    if (entry.isFile())
      return [
        {
          name: entry.name,
          text: fs.readFileSync(path.join(directory, entry.name), "utf8"),
        },
      ];
    if (entry.isDirectory())
      return fs
        .readdirSync(path.join(directory, entry.name))
        .filter((name) =>
          fs.statSync(path.join(directory, entry.name, name)).isFile(),
        )
        .map((name) => ({
          name: `${entry.name}/${name}`,
          text: fs.readFileSync(path.join(directory, entry.name, name), "utf8"),
        }));
    return [];
  });
}
async function readUrl(url) {
  const response = await fetch(url, {
    headers: { "User-Agent": "openfile-notice-audit" },
    signal: AbortSignal.timeout(30000),
  });
  if (!response.ok) throw new Error(`${response.status} reading ${url}`);
  return response.text();
}
const treeCache = new Map();
async function upstreamNotices(pkg) {
  const directory = path.dirname(pkg.manifest_path);
  const file = path.join(cache, `${pkg.name}-${pkg.version}.json`);
  if (fs.existsSync(file)) return JSON.parse(fs.readFileSync(file, "utf8"));
  if (!args.includes("--fetch"))
    throw new Error(
      `Missing cached upstream notices for ${pkg.name}; rerun with --fetch`,
    );
  const vcs = JSON.parse(
    fs.readFileSync(path.join(directory, ".cargo_vcs_info.json"), "utf8"),
  );
  const repository = pkg.repository?.match(
    /^https:\/\/github.com\/([^/]+\/[^/]+?)(?:\.git)?\/?$/,
  )?.[1];
  if (!repository)
    throw new Error(`No pinned GitHub notice source for ${pkg.name}`);
  const key = `${repository}/${vcs.git.sha1}`;
  if (!treeCache.has(key))
    treeCache.set(
      key,
      JSON.parse(
        await readUrl(
          `https://api.github.com/repos/${repository}/git/trees/${vcs.git.sha1}?recursive=1`,
        ),
      ),
    );
  const tree = treeCache.get(key);
  if (tree.truncated) throw new Error(`Truncated notice inventory for ${key}`);
  let ancestor = vcs.path_in_vcs;
  let files = [];
  for (;;) {
    files = tree.tree.filter(
      (entry) =>
        entry.type === "blob" &&
        path.posix.dirname(entry.path) === (ancestor || ".") &&
        (licenseName(path.posix.basename(entry.path)) ||
          (pkg.name === "r-efi" &&
            path.posix.basename(entry.path) === "AUTHORS")),
    );
    if (files.length || !ancestor) break;
    ancestor = path.posix.dirname(ancestor);
    if (ancestor === ".") ancestor = "";
  }
  if (!files.length)
    throw new Error(`No upstream notices for ${pkg.name} at ${key}`);
  const notices = [];
  for (const entry of files) {
    const source = `https://raw.githubusercontent.com/${repository}/${vcs.git.sha1}/${entry.path}`;
    notices.push({ name: entry.path, source, text: await readUrl(source) });
  }
  fs.writeFileSync(file, JSON.stringify(notices, null, 2) + "\n");
  return notices;
}
function store(notices, origin) {
  return notices.map((notice) => {
    const bytes = Buffer.from(notice.text);
    const sha256 = digest(bytes);
    const filename = `licenses/notices/${sha256}.txt`;
    fs.writeFileSync(filename, bytes);
    return {
      path: filename,
      sha256,
      origin: notice.source || `${origin}/${notice.name}`,
    };
  });
}
const inventory = {
  schema: 1,
  scope:
    "All locked external Rust crates (including target/build dependencies) and production npm packages; no dependency source or engine binaries bundled.",
  locks: Object.fromEntries(
    ["Cargo.lock", "package-lock.json"].map((file) => [
      file,
      digest(fs.readFileSync(file)),
    ]),
  ),
  rust: [],
  npm: [],
};
for (const pkg of metadata.packages
  .filter((pkg) => pkg.source)
  .sort((a, b) =>
    `${a.name}@${a.version}`.localeCompare(`${b.name}@${b.version}`),
  )) {
  let notices = localNotices(path.dirname(pkg.manifest_path));
  let origin = `crate:${pkg.name}@${pkg.version}`;
  if (!notices.length && pkg.name.startsWith("winapi-")) {
    const shared = metadata.packages.find((pkg) => pkg.name === "winapi");
    notices = localNotices(path.dirname(shared.manifest_path));
    origin = `crate:winapi@${shared.version} (shared winapi-rs license)`;
  } else if (!notices.length && pkg.name === "libappindicator-sys") {
    const shared = metadata.packages.find(
      (pkg) => pkg.name === "libappindicator",
    );
    notices = localNotices(path.dirname(shared.manifest_path));
    origin = `crate:libappindicator@${shared.version} (shared project license)`;
  }
  if (!notices.length && pkg.name === "selectors") {
    const source = fs.readFileSync(
      path.join(path.dirname(pkg.manifest_path), "lib.rs"),
      "utf8",
    );
    const header = source.match(/^\/\*[\s\S]*?\*\//)?.[0];
    if (!header?.includes("Mozilla Public"))
      throw new Error("Missing selectors MPL source notice");
    const cacheFile = path.join(cache, "MPL-2.0.txt");
    const url = "https://www.mozilla.org/media/MPL/2.0/index.txt";
    if (!fs.existsSync(cacheFile)) {
      if (!args.includes("--fetch"))
        throw new Error("Missing cached MPL-2.0 license");
      fs.writeFileSync(cacheFile, await readUrl(url));
    }
    notices = [
      { name: "lib.rs license header", text: header + "\n" },
      {
        name: "MPL-2.0",
        source: url,
        text: fs.readFileSync(cacheFile, "utf8"),
      },
    ];
  }
  if (!notices.length) notices = await upstreamNotices(pkg);
  if (!pkg.license || !notices.length)
    throw new Error(`Missing dependency license: ${pkg.name}`);
  inventory.rust.push({
    name: pkg.name,
    version: pkg.version,
    license: pkg.license,
    repository: pkg.repository,
    notices: store(notices, origin),
  });
}
const lock = JSON.parse(fs.readFileSync("package-lock.json", "utf8"));
for (const [name, pkg] of Object.entries(lock.packages)
  .filter(([name, pkg]) => name && !pkg.dev)
  .sort()) {
  const notices = localNotices(name);
  if (name === "node_modules/@tauri-apps/plugin-dialog") {
    notices.push(
      ...localNotices("node_modules/@tauri-apps/api")
        .filter((notice) =>
          ["LICENSE-MIT", "LICENSE-APACHE-2.0"].includes(notice.name),
        )
        .map((notice) => ({
          ...notice,
          source: `npm:@tauri-apps/api@${lock.packages["node_modules/@tauri-apps/api"].version}/${notice.name} (shared Tauri license text; plugin copyright retained in SPDX notice)`,
        })),
    );
  }
  if (!pkg.license || !notices.length)
    throw new Error(`Missing npm license: ${name}`);
  inventory.npm.push({
    name,
    version: pkg.version,
    license: pkg.license,
    notices: store(notices, `npm:${name.slice(13)}@${pkg.version}`),
  });
}
fs.writeFileSync(
  "licenses/dependencies.json",
  JSON.stringify(inventory, null, 2) + "\n",
);
const used = new Set(
  [...inventory.rust, ...inventory.npm].flatMap((pkg) =>
    pkg.notices.map((notice) => path.basename(notice.path)),
  ),
);
for (const name of fs.readdirSync("licenses/notices"))
  if (/^[a-f0-9]{64}\.txt$/.test(name) && !used.has(name))
    fs.unlinkSync(path.join("licenses/notices", name));
console.log(
  `Recorded ${inventory.rust.length} Rust and ${inventory.npm.length} npm packages; ${used.size} distinct notice files.`,
);
