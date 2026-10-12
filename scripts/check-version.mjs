// Fails when the app version differs between package.json, package-lock.json,
// tauri.conf.json, Cargo.toml and Cargo.lock. With `--tag v1.2.3`, also fails
// unless that version is 1.2.3 (release tags must match what gets built).
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

export const root = join(dirname(fileURLToPath(import.meta.url)), "..");

export const VERSION_FILES = [
  "package.json",
  "package-lock.json",
  "src-tauri/tauri.conf.json",
  "src-tauri/Cargo.toml",
  "src-tauri/Cargo.lock",
];

export const CARGO_TOML_VERSION = /^(\[package\]\n(?:(?!\[)[^\n]*\n)*?version = ")([^"]+)(")/m;
export const CARGO_LOCK_VERSION = /(\[\[package\]\]\nname = "castle-keeper"\nversion = ")([^"]+)(")/;

const read = (file) => readFileSync(join(root, file), "utf8");

/** Every place the app version is recorded, as [label, version]. */
export function readVersions() {
  const lock = JSON.parse(read("package-lock.json"));
  return [
    ["package.json", JSON.parse(read("package.json")).version],
    ["package-lock.json", lock.version],
    ['package-lock.json packages[""]', lock.packages[""].version],
    ["src-tauri/tauri.conf.json", JSON.parse(read("src-tauri/tauri.conf.json")).version],
    ["src-tauri/Cargo.toml", read("src-tauri/Cargo.toml").match(CARGO_TOML_VERSION)?.[2]],
    ["src-tauri/Cargo.lock", read("src-tauri/Cargo.lock").match(CARGO_LOCK_VERSION)?.[2]],
  ];
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const tagIndex = process.argv.indexOf("--tag");
  const tag = tagIndex === -1 ? undefined : process.argv[tagIndex + 1];
  const versions = readVersions();
  const expected = tag ? tag.replace(/^v/, "") : versions[0][1];

  let failed = false;
  for (const [label, version] of versions) {
    const ok = version === expected;
    console.log(`${ok ? "ok      " : "MISMATCH"} ${label}: ${version ?? "(not found)"}`);
    failed ||= !ok;
  }
  if (failed) {
    console.error(
      tag
        ? `\nTag ${tag} doesn't match the app version. Tag releases with \`npm run release <version>\`.`
        : "\nThe app version differs between files. Set it with `npm run release <version>`.",
    );
    process.exit(1);
  }
}
