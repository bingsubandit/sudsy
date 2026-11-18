#!/usr/bin/env python3
"""Renders the package-manager packages of one release from its CLI archives.

Usage: scripts/package-channels.py <npm|pypi|homebrew|scoop|winget|all> <version> <assets dir> <out dir> [tag]
The assets dir holds the release's `sudsy-<version>-<platform>.{tar.gz,zip}` and `SHA256SUMS`;
when it has no SHA256SUMS they are fetched with `gh release download <tag>`. Nothing is published here. Templates live in packaging/ (`@VERSION@`,
`@URL <platform>@`, `@SHA <platform>@`). Layout of the output: see docs/development/ci-release.md.
"""
import base64
import hashlib
import re
import shutil
import subprocess
import sys
import tarfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PKG = ROOT / "packaging"
REPO = "bingsubandit/sudsy"
PLATFORMS = ["linux-x64", "linux-arm64", "darwin-x64", "darwin-arm64", "win32-x64", "win32-arm64"]
# Wheel platform tags (glibc 2.39 = the ubuntu-24.04 runners that build the binaries).
WHEEL_TAG = {
    "linux-x64": "manylinux_2_39_x86_64", "linux-arm64": "manylinux_2_39_aarch64",
    "darwin-x64": "macosx_10_12_x86_64", "darwin-arm64": "macosx_11_0_arm64",
    "win32-x64": "win_amd64", "win32-arm64": "win_arm64",
}
NPM_SCOPE = "@sudsylint"
DESC = "Formatter and linter for C#, JSON, YAML and more"


def archive_name(version, platform):
    return f"sudsy-{version}-{platform}." + ("zip" if platform.startswith("win32") else "tar.gz")


def sums(assets):
    return {name: h for h, name in (l.split() for l in (assets / "SHA256SUMS").read_text().splitlines() if l.strip())}


def unpack(assets, version, platform, into):
    """The archive's content directory (`sudsy[.exe]`, `csharp-worker/`, LICENSE, CHANGELOG.md)."""
    path = assets / archive_name(version, platform)
    if path.suffix == ".zip":
        zipfile.ZipFile(path).extractall(into)
    else:
        tarfile.open(path).extractall(into, filter="tar")
    return into / f"sudsy-{version}-{platform}"


def render(template, version, urls, hashes):
    text = (PKG / template).read_text()
    text = text.replace("@VERSION@", version)
    text = re.sub(r"@URL ([\w-]+)@", lambda m: urls[m[1]], text)
    return re.sub(r"@SHA ([\w-]+)@", lambda m: hashes[m[1]], text)


def write_text(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, newline="\n")


def npm(version, assets, out, tmp, **_):
    exe = lambda p: "sudsy.exe" if p.startswith("win32") else "sudsy"
    for p in PLATFORMS:
        os_, cpu = p.split("-")
        d = out / f"cli-{p}"
        shutil.copytree(unpack(assets, version, p, tmp / p), d)
        (d / "CHANGELOG.md").unlink()
        write_json(d / "package.json", {
            "name": f"{NPM_SCOPE}/cli-{p}", "version": version, "description": f"{DESC} (binary for {p})",
            "license": "MIT", "repository": f"github:{REPO}", "os": [os_], "cpu": [cpu],
            "files": [exe(p), "csharp-worker", "LICENSE"],
            "publishConfig": {"access": "public"},
        })
    d = out / "sudsy"
    (d / "bin").mkdir(parents=True)
    shutil.copy(PKG / "npm/bin.js", d / "bin/sudsy.js")
    (d / "bin/sudsy.js").chmod(0o755)
    shutil.copy(ROOT / "LICENSE", d / "LICENSE")
    shutil.copy(ROOT / "README.md", d / "README.md")
    write_json(d / "package.json", {
        "name": "sudsy", "version": version, "description": DESC, "license": "MIT",
        "repository": f"github:{REPO}", "homepage": f"https://github.com/{REPO}",
        "bin": {"sudsy": "bin/sudsy.js"}, "files": ["bin", "LICENSE", "README.md"],
        "optionalDependencies": {f"{NPM_SCOPE}/cli-{p}": version for p in PLATFORMS},
    })


def write_json(path, obj):
    import json
    write_text(path, json.dumps(obj, indent=2) + "\n")


def record_hash(data):
    return "sha256=" + base64.urlsafe_b64encode(hashlib.sha256(data).digest()).rstrip(b"=").decode()


def pypi(version, assets, out, tmp, **_):
    dist = f"sudsy-{version}.dist-info"
    for p in PLATFORMS:
        src = unpack(assets, version, p, tmp / p)
        files = {}  # wheel path -> (bytes, mode)
        for f in sorted(src.rglob("*")):
            rel = f.relative_to(src).as_posix()
            if f.is_file() and rel not in ("CHANGELOG.md",):
                if rel == "LICENSE":
                    files[f"{dist}/licenses/LICENSE"] = (f.read_bytes(), 0o644)
                else:
                    files[f"sudsy/bin/{rel}"] = (f.read_bytes(), 0o755 if f.stat().st_mode & 0o111 or rel == "sudsy.exe" else 0o644)
        files["sudsy/__init__.py"] = ((PKG / "pypi/__init__.py").read_bytes(), 0o644)
        tag = f"py3-none-{WHEEL_TAG[p]}"
        files[f"{dist}/METADATA"] = (
            f"Metadata-Version: 2.4\nName: sudsy\nVersion: {version}\nSummary: {DESC}\n"
            f"License-Expression: MIT\nLicense-File: LICENSE\nProject-URL: Homepage, https://github.com/{REPO}\n"
            f"Requires-Python: >=3.8\n\n{(ROOT / 'README.md').read_text()}".encode(), 0o644)
        files[f"{dist}/WHEEL"] = (f"Wheel-Version: 1.0\nGenerator: scripts/package-channels.py\nRoot-Is-Purelib: false\nTag: {tag}\n".encode(), 0o644)
        files[f"{dist}/entry_points.txt"] = (b"[console_scripts]\nsudsy = sudsy:main\n", 0o644)
        record = "".join(f"{n},{record_hash(b)},{len(b)}\n" for n, (b, _) in files.items()) + f"{dist}/RECORD,,\n"
        files[f"{dist}/RECORD"] = (record.encode(), 0o644)
        out.mkdir(parents=True, exist_ok=True)
        with zipfile.ZipFile(out / f"sudsy-{version}-{tag}.whl", "w", zipfile.ZIP_DEFLATED) as z:
            for n, (b, mode) in files.items():
                info = zipfile.ZipInfo(n, (2020, 1, 1, 0, 0, 0))
                info.external_attr = (0o100000 | mode) << 16
                info.compress_type = zipfile.ZIP_DEFLATED
                z.writestr(info, b)


def manifests(templates, dest, version, assets, tag, platforms, hash_case=str):
    names = sums(assets)
    urls = {p: f"https://github.com/{REPO}/releases/download/{tag}/{archive_name(version, p)}" for p in platforms}
    hashes = {p: hash_case(names[archive_name(version, p)]) for p in platforms}
    for template, name in templates:
        text = render(template, version, urls, hashes)
        write_text(dest / name, text)


def homebrew(version, assets, out, tag, **_):
    manifests([("homebrew/sudsy.rb", "Formula/sudsy.rb")], out, version, assets, tag,
              ["darwin-x64", "darwin-arm64", "linux-x64", "linux-arm64"])


def scoop(version, assets, out, tag, **_):
    manifests([("scoop/sudsy.json", "bucket/sudsy.json")], out, version, assets, tag, ["win32-x64", "win32-arm64"])


def winget(version, assets, out, tag, **_):
    manifests([(f"winget/Coox.Sudsy{s}.yaml", f"manifests/c/Coox/Sudsy/{version}/Coox.Sudsy{s}.yaml")
               for s in ("", ".installer", ".locale.en-US")], out, version, assets, tag,
              ["win32-x64", "win32-arm64"], str.upper)  # winget wants uppercase hashes


CHANNELS = {"npm": npm, "pypi": pypi, "homebrew": homebrew, "scoop": scoop, "winget": winget}


def main():
    if len(sys.argv) not in (5, 6):
        sys.exit(__doc__)
    channel, version, assets, out = sys.argv[1], sys.argv[2], Path(sys.argv[3]).resolve(), Path(sys.argv[4]).resolve()
    tag = sys.argv[5] if len(sys.argv) == 6 else f"v{version}"
    if not (assets / "SHA256SUMS").exists():
        subprocess.run(["gh", "release", "download", tag, "--dir", str(assets), "--clobber",
                        "-p", f"sudsy-{version}-*.tar.gz", "-p", f"sudsy-{version}-*.zip", "-p", "SHA256SUMS"], check=True)
    for name in CHANNELS if channel == "all" else [channel]:
        if name not in CHANNELS:
            sys.exit(f"unknown channel {name}")
        tmp = out / ".tmp" / name
        tmp.mkdir(parents=True, exist_ok=True)
        CHANNELS[name](version=version, assets=assets, out=out / name, tmp=tmp, tag=tag)
        shutil.rmtree(tmp)
        print(f"{name}: {out / name}")
    shutil.rmtree(out / ".tmp", ignore_errors=True)


if __name__ == "__main__":
    main()
