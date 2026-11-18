#!/usr/bin/env node
// Runs the `sudsy` binary of the platform package installed next to this one.
const { spawnSync } = require("node:child_process");
const path = require("node:path");

const platform = `${process.platform}-${process.arch}`;
let exe;
try {
  const dir = path.dirname(require.resolve(`@sudsylint/cli-${platform}/package.json`));
  exe = path.join(dir, process.platform === "win32" ? "sudsy.exe" : "sudsy");
} catch {
  console.error(`sudsy: no binary package for ${platform} (are optionalDependencies disabled?)`);
  process.exit(1);
}
const r = spawnSync(exe, process.argv.slice(2), { stdio: "inherit" });
if (r.error) {
  console.error(`sudsy: ${r.error.message}`);
  process.exit(1);
}
process.exit(r.status ?? 1);
