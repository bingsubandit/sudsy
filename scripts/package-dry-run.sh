#!/usr/bin/env bash
# Renders every package-manager package from a published release's assets into a temp dir and
# checks them; publishes nothing. Usage: scripts/package-dry-run.sh [tag] [output dir]
# (default: the latest release, a fresh temp dir). Needs gh, python3 and node.
set -euo pipefail
cd "$(dirname "$0")/.."
tag=${1:-$(gh release view --json tagName -q .tagName)}
out=${2:-$(mktemp -d)}
version=${tag#v}
python3 -I scripts/package-channels.py all "$version" "$out/assets" "$out/packages" "$tag"
rm -rf "$out/assets"

# Checks: JSON parses, wheels are consistent, no placeholder is left, the npm shim runs the
# binary of this machine.
python3 -m json.tool "$out/packages/scoop/bucket/sudsy.json" > /dev/null
python3 -m json.tool "$out/packages/npm/sudsy/package.json" > /dev/null
! grep -rIl '@VERSION@\|@URL \|@SHA ' "$out/packages"
python3 -I - "$out/packages/pypi" <<'EOF'
import sys, zipfile, pathlib
wheels = sorted(pathlib.Path(sys.argv[1]).glob("*.whl"))
assert len(wheels) == 6, wheels
for w in wheels:
    z = zipfile.ZipFile(w)
    assert z.testzip() is None
    names = z.namelist()
    assert "sudsy/__init__.py" in names and any(n.startswith("sudsy/bin/csharp-worker/") for n in names), w
EOF
host=$(node -p 'process.platform + "-" + process.arch')
if [ -d "$out/packages/npm/cli-$host" ]; then
  mkdir -p "$out/packages/npm/sudsy/node_modules/@sudsylint"
  ln -s "$out/packages/npm/cli-$host" "$out/packages/npm/sudsy/node_modules/@sudsylintlint/cli-$host"
  node "$out/packages/npm/sudsy/bin/sudsy.js" --version
  rm -r "$out/packages/npm/sudsy/node_modules"
fi
echo "rendered into $out/packages"
