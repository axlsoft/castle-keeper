// Fails when a Tauri Rust crate and its npm counterpart drift apart on
// major.minor. `tauri build` refuses to run in that state, so catch it early.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

const lock = readFileSync(join(root, "src-tauri", "Cargo.lock"), "utf8");
const crates = new Map();
for (const m of lock.matchAll(/\[\[package\]\]\nname = "([^"]+)"\nversion = "([^"]+)"/g)) {
  crates.set(m[1], m[2]);
}

const pairs = [["tauri", "@tauri-apps/api"]];
for (const name of crates.keys()) {
  const plugin = name.match(/^tauri-plugin-(.+)$/);
  if (plugin) pairs.push([name, `@tauri-apps/plugin-${plugin[1]}`]);
}

const minor = (v) => v.split(".").slice(0, 2).join(".");
let failed = false;
for (const [crate, pkg] of pairs) {
  let npmVersion;
  try {
    npmVersion = JSON.parse(readFileSync(join(root, "node_modules", pkg, "package.json"), "utf8")).version;
  } catch {
    continue; // Rust-only plugin, nothing to compare.
  }
  const rustVersion = crates.get(crate);
  const ok = minor(rustVersion) === minor(npmVersion);
  console.log(`${ok ? "ok      " : "MISMATCH"} ${crate} ${rustVersion} <-> ${pkg} ${npmVersion}`);
  failed ||= !ok;
}

if (failed) {
  console.error("\nTauri crates and npm packages must share major.minor. Bump them together.");
  process.exit(1);
}
