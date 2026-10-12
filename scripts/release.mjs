// Sets the app version everywhere it is recorded, commits that change and
// creates the matching annotated tag, all locally. Pushing the tag starts the
// release build, which publishes a draft GitHub Release for review.
//
//   npm run release 1.2.3              bump, commit and tag v1.2.3
//   npm run release -- 1.2.3 --dry-run show what would change
//
// Versions are MAJOR.MINOR.PATCH, optionally with a numeric pre-release
// (1.2.3-1): Windows MSI installers reject anything else.
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { CARGO_LOCK_VERSION, CARGO_TOML_VERSION, VERSION_FILES, readVersions, root } from "./check-version.mjs";

const args = process.argv.slice(2);
const dryRun = args.includes("--dry-run");
const version = args.find((arg) => !arg.startsWith("--"))?.replace(/^v/, "");
const tag = `v${version}`;

function fail(message) {
  console.error(message);
  process.exit(1);
}

const git = (...gitArgs) => execFileSync("git", gitArgs, { cwd: root, encoding: "utf8" }).trim();

function parse(v) {
  const m = v?.match(/^(\d+)\.(\d+)\.(\d+)(?:-(\d+))?$/);
  return m && [Number(m[1]), Number(m[2]), Number(m[3]), m[4] === undefined ? Infinity : Number(m[4])];
}

function compare(a, b) {
  for (let i = 0; i < 4; i++) if (a[i] !== b[i]) return a[i] < b[i] ? -1 : 1;
  return 0;
}

if (!version) fail("Usage: npm run release <version>   e.g. npm run release 0.2.0");
const next = parse(version);
if (!next) fail(`"${version}" isn't MAJOR.MINOR.PATCH (optionally -N, e.g. 1.2.3-1).`);

const versions = readVersions();
const current = versions[0][1];
const mismatched = versions.filter(([, v]) => v !== current);
if (mismatched.length) fail("The version already differs between files; run `npm run check:version`.");
if (compare(next, parse(current)) <= 0) fail(`${version} must be newer than the current ${current}.`);

const branch = git("branch", "--show-current");
if (branch !== "main") fail(`Releases are tagged on main; you're on ${branch || "a detached HEAD"}.`);
if (git("status", "--porcelain", "--", ...VERSION_FILES))
  fail(`Commit or discard changes to these files first:\n  ${VERSION_FILES.join("\n  ")}`);
try {
  git("rev-parse", "--quiet", "--verify", `refs/tags/${tag}`);
  fail(`Tag ${tag} already exists.`);
} catch {
  // No such tag: good.
}

console.log(`${current} -> ${version}`);
if (dryRun) {
  console.log(`Would update:\n  ${VERSION_FILES.join("\n  ")}\nthen commit and tag ${tag}.`);
  process.exit(0);
}

// JSON files are rewritten whole, so make sure that keeps their formatting.
function updateJson(file, apply) {
  const path = join(root, file);
  const text = readFileSync(path, "utf8");
  const data = JSON.parse(text);
  if (JSON.stringify(data, null, 2) + "\n" !== text)
    fail(`${file} isn't in standard 2-space JSON format; refusing to rewrite it.`);
  apply(data);
  writeFileSync(path, JSON.stringify(data, null, 2) + "\n");
}

function updateText(file, pattern) {
  const path = join(root, file);
  const text = readFileSync(path, "utf8");
  if (!pattern.test(text)) fail(`Couldn't find the version in ${file}.`);
  writeFileSync(path, text.replace(pattern, `$1${version}$3`));
}

updateJson("package.json", (pkg) => (pkg.version = version));
updateJson("package-lock.json", (lock) => {
  lock.version = version;
  lock.packages[""].version = version;
});
updateJson("src-tauri/tauri.conf.json", (conf) => (conf.version = version));
updateText("src-tauri/Cargo.toml", CARGO_TOML_VERSION);
updateText("src-tauri/Cargo.lock", CARGO_LOCK_VERSION);

const stale = readVersions().filter(([, v]) => v !== version);
if (stale.length) fail(`Not updated: ${stale.map(([label]) => label).join(", ")}`);

git("add", "--", ...VERSION_FILES);
git("commit", "--quiet", "-m", `chore(release): ${tag}`);
git("tag", "--annotate", tag, "-m", `Castle Keeper ${version}`);

console.log(`Committed and tagged ${tag}. Nothing has been pushed. To release:

  git push --atomic origin main ${tag}

CI builds the installers and publishes a draft release to review and publish.
To undo before pushing: git tag -d ${tag} && git reset --keep HEAD~1`);
