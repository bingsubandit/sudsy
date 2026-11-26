"""Compares `sudsy format` with an upstream formatter on tests/parity/<tool>/ (run by `scripts/parity.sh format`).

Usage: parity_format.py <tool> <sudsy-binary> <target-dir>

The tools are the entries of parity_format.json (documented in docs/ai/development/parity.md): version
(from docs/ai/parity/<catalog>.yaml), how to install it, its command, how it takes its input and which
files are the fixtures. Every fixture is formatted by the tool and by Sudsy (with the fixture
folder's sudsy.yaml, which extends the matching preset); the outputs must be equal. A fixture listed
in the folder's `differences.yaml` may differ; it is reported as
known; the file is `differences.yaml` (docs/ai/development/parity.md): named differences, the fixtures that
differ, and the `ratchet` minimums of the groups (a corpus too large for entries per file) that are only counted.
"""
import difflib
import hashlib
import io
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import urllib.request
import zipfile
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import yaml

import parity_landing
import parity_wrap

ROOT = Path(__file__).resolve().parent.parent


# Names of the files left out for not being UTF-8: they may carry a named gap (`non-utf8-input`).
NON_UTF8 = set()

def run(cmd, cwd, stdin=None, check=True, env=None):
    data = stdin.encode("utf-8") if stdin is not None else None
    cmd = [shutil.which(cmd[0]) or cmd[0], *map(str, cmd[1:])]
    r = subprocess.run(cmd, cwd=cwd, input=data, capture_output=True, check=False, env=env)
    if check and r.returncode != 0:
        raise SystemExit(f"{' '.join(map(str, cmd))} failed:\n{r.stderr.decode()}{r.stdout.decode()}")
    return r


def tools():
    return json.loads((ROOT / "scripts/parity_tools.json").read_text())


def tool_command(config, prefix, **names):
    """The `command` of a tool entry with its placeholders filled (`{prefix}`, `{python}`, `{root}` and `names`),
    and the environment of the run: the process's plus the entry's `env` (a Python tool: `{python} -m <tool>`, PYTHONPATH `{prefix}`)."""
    command = [str(a).format(prefix=prefix, python=sys.executable, root=ROOT, **names) for a in config["command"]]
    return command, {**os.environ, **{k: v.format(prefix=prefix) for k, v in config.get("env", {}).items()}}


def catalog_version(name):
    text = (ROOT / "docs/ai/parity" / f"{name}.yaml").read_text(encoding="utf-8")
    return re.search(r'^version: "([^ "]+)', text, re.M).group(1)


def executable(prefix, name):
    exe = prefix / f"{name}.exe"
    return exe if exe.exists() else prefix / name


def ensure_pwsh(version, parity):
    """`pwsh` from PATH, else the portable PowerShell release of that version (Linux) unpacked below `parity`, put on PATH (the runs call `pwsh`)."""
    if shutil.which("pwsh"):
        return
    if not sys.platform.startswith("linux"):
        sys.exit("parity: no pwsh on PATH (a portable build is fetched on Linux only)")
    root = parity / f"pwsh-{version}"
    if not (root / "pwsh").exists():
        arch = "arm64" if platform.machine().lower() in ("aarch64", "arm64") else "x64"
        url = f"https://github.com/PowerShell/PowerShell/releases/download/v{version}/powershell-{version}-linux-{arch}.tar.gz"
        shutil.rmtree(root, ignore_errors=True)
        root.mkdir(parents=True)
        run(["bash", "-c", f"curl -fsSL '{url}' | tar xz -C '{root}'"], ROOT)
        (root / "pwsh").chmod(0o755)
    os.environ["PATH"] = f"{root}{os.pathsep}{os.environ['PATH']}"


def install(spec, catalog, version, prefix):
    """Installs the tool into prefix unless it is there."""
    if "npm" in spec:
        wanted = [spec["npm"], *(w.rsplit("@", 1)[0] for w in spec.get("with", []))]
        if not all((prefix / "node_modules" / w).is_dir() for w in wanted):
            prefix.mkdir(parents=True, exist_ok=True)
            run(["npm", "install", "--silent", "--no-audit", "--no-fund", "--prefix", prefix, *spec.get("with", []), f"{spec['npm']}@{version}"], ROOT)
    elif "pip" in spec:  # a Python tool, run with PYTHONPATH=prefix
        if not (prefix / spec.get("dir", spec["pip"])).is_dir():
            if subprocess.run([sys.executable, "-m", "pip", "--version"], capture_output=True).returncode == 0:
                run([sys.executable, "-m", "pip", "install", "--quiet", "--target", prefix, f"{spec['pip']}=={version}"], ROOT)
            elif shutil.which("uv"):  # no pip, but uv: it installs the dependencies too (cmakelang needs six)
                run(["uv", "pip", "install", "--quiet", "--link-mode=copy", "--target", prefix, f"{spec['pip']}=={version}"], ROOT)
            else:  # no pip on the host: the tool's wheel from PyPI, unpacked (a tool without dependencies, as ruff)
                files = json.load(urllib.request.urlopen(f"https://pypi.org/pypi/{spec['pip']}/{version}/json"))["urls"]
                machine = platform.machine().lower()
                url = next(f["url"] for f in files if f["filename"].endswith(".whl") and ("none-any" in f["filename"] or ("manylinux" in f["filename"] and machine in f["filename"])))
                zipfile.ZipFile(io.BytesIO(urllib.request.urlopen(url).read())).extractall(prefix)
                (prefix / "bin").mkdir(exist_ok=True)  # where `pip --target` puts the scripts
                for script in prefix.glob("*.data/scripts/*"):
                    shutil.move(script, prefix / "bin" / script.name)
                    (prefix / "bin" / script.name).chmod(0o755)
    elif "gem" in spec:  # a Ruby gem: Ruby from PATH, else the portable build of ruby/ruby-builder (Linux); `prefix/bin/<gem>` runs it
        exe = prefix / "bin" / spec["gem"]
        if not exe.exists():
            ruby = shutil.which("ruby")
            env = ""
            if ruby is None:
                if not sys.platform.startswith("linux"):
                    sys.exit("parity: no ruby on PATH (a portable build is fetched on Linux only)")
                ruby_root = prefix / "ruby"
                if not (ruby_root / "bin" / "ruby").exists():
                    arch = "-arm64" if platform.machine().lower() in ("aarch64", "arm64") else ""
                    url = f"https://github.com/ruby/ruby-builder/releases/download/toolcache/ruby-{spec['ruby']}-ubuntu-24.04{arch}.tar.gz"
                    shutil.rmtree(prefix / "unpack", ignore_errors=True)
                    (prefix / "unpack").mkdir(parents=True)
                    run(["bash", "-c", f"curl -fsSL '{url}' | tar xz -C '{prefix / 'unpack'}'"], ROOT)
                    next((prefix / "unpack").iterdir()).rename(ruby_root)  # one folder (`arm64`, `x64`), built for another prefix
                lib = next((ruby_root / "lib/ruby").glob("3.*"))  # its paths are baked in: point Ruby at its own
                dirs = ":".join(str(d) for d in (lib, *lib.glob("*-linux"), ruby_root / "lib/ruby/site_ruby" / lib.name))
                env = f'export LD_LIBRARY_PATH="{ruby_root}/lib" RUBYLIB="{dirs}" GEM_PATH="{ruby_root}/lib/ruby/gems/{lib.name}:{prefix}/gems"\n'
                ruby = str(ruby_root / "bin" / "ruby")
            gems = prefix / "gems"
            run(["bash", "-c", f'{env}export GEM_HOME="{gems}"; "{ruby}" "{Path(ruby).parent}/gem" install --no-document {spec["gem"]} -v {version}'], ROOT)
            exe.parent.mkdir(parents=True, exist_ok=True)
            exe.write_text(f'#!/bin/sh\n{env}export GEM_HOME="{gems}"\nexec "{ruby}" "{gems}/bin/{spec["gem"]}" "$@"\n')
            exe.chmod(0o755)
    elif "dotnet-project" in spec:  # a .NET project of this repository, built into prefix at the tool's version
        stamp = hashlib.sha256(b"".join(f.read_bytes() for f in sorted((ROOT / spec["dotnet-project"]).glob("*.cs*")))).hexdigest()  # a changed source rebuilds
        if not (prefix / spec["dll"]).exists() or not (prefix / ".source-hash").exists() or (prefix / ".source-hash").read_text() != stamp:
            run(["dotnet", "build", "-c", "Release", "--nologo", "-v:q", f"-p:AnalyzerPackage={spec['package']}", f"-p:AnalyzerVersion={version}", "-o", prefix, spec["dotnet-project"]], ROOT)
            (prefix / ".source-hash").write_text(stamp)
    elif "dotnet-tool" in spec:
        if not executable(prefix, spec["exe"]).exists():
            run(["dotnet", "tool", "install", spec["dotnet-tool"], "--version", version, "--tool-path", prefix], ROOT)
    elif "psmodule" in spec:
        ensure_pwsh(spec["pwsh"], prefix.parent)
        if not (prefix / spec["psmodule"]).is_dir():
            prefix.mkdir(parents=True, exist_ok=True)
            run(["pwsh", "-NoProfile", "-Command", f"Save-Module {spec['psmodule']} -RequiredVersion {version} -Path '{prefix}'"], ROOT)
    elif "jar" in spec:  # a shaded JVM tool: the pinned jar of Maven Central (checksum verified) and a launcher on the shared JRE
        exe = executable(prefix, spec["exe"])
        if not exe.exists():
            prefix.mkdir(parents=True, exist_ok=True)
            jar = prefix / f"{spec['exe']}.jar"
            run(["curl", "-fsSL", "-o", jar, spec["jar"].format(version=version)], ROOT)
            if hashlib.sha256(jar.read_bytes()).hexdigest() != spec["sha256"]:
                jar.unlink()
                sys.exit(f"parity: checksum mismatch for {spec['jar'].format(version=version)}")
            java = run(["bash", "scripts/lib/jre.sh", prefix.parent], ROOT).stdout.decode().strip()
            exe.write_text(f'#!/bin/sh\nexec "{java}" -jar "{jar}" "$@"\n')
            exe.chmod(0o755)
    elif "release" in spec:
        for name, url in spec.get("files", {}).items():  # more downloads next to the executable (a phar for a static interpreter)
            if not (prefix / name).exists():
                prefix.mkdir(parents=True, exist_ok=True)
                run(["curl", "-fsSL", "-o", prefix / name, url.format(version=version)], ROOT)
        if not executable(prefix, spec["exe"]).exists():
            prefix.mkdir(parents=True, exist_ok=True)
            system = {"win32": "windows", "linux": "linux", "darwin": "darwin"}.get(sys.platform, sys.platform)
            key = system if system == "windows" else f"{system}-{platform.machine().lower()}"
            asset = (spec["assets"].get(key) or sys.exit(f"parity: no {catalog} release for {key}")).format(version=version)
            archive = prefix / asset
            run(["curl", "-fsSL", "-o", archive, spec["release"].format(version=version, asset=asset)], ROOT)
            if asset.endswith((".zip", ".tar.gz", ".tar.xz")):
                shutil.unpack_archive(archive, prefix)
                if executable(prefix, spec["exe"]).exists():
                    executable(prefix, spec["exe"]).chmod(0o755)
                if not executable(prefix, spec["exe"]).exists():  # inside a folder of the archive
                    found = next(p for n in (spec["exe"] + ".exe", spec["exe"]) for p in prefix.rglob(n))
                    found.replace(prefix / found.name)
                    (prefix / found.name).chmod(0o755)
            elif asset.endswith(".gz"):
                target = prefix / spec["exe"]
                run(["gunzip", "-f", archive], ROOT)
                (prefix / asset[: -len(".gz")]).rename(target)
                target.chmod(0o755)
            else:  # the asset is the executable
                target = prefix / (spec["exe"] + (".exe" if asset.endswith(".exe") else ""))
                archive.replace(target)
                target.chmod(0o755)
        if "composer" in spec:
            composer_vendors(spec, prefix)


def composer_vendors(spec, prefix):
    """The dependencies of the corpus projects (`composer`: project -> the folder of its composer.json, and its composer.lock
    when it has one: composer installs from it, else resolves the exact versions the file pins), installed with the pinned
    composer.phar into prefix/vendor/<project>/vendor (the project's own `vendor/` below a folder of its own, with an empty `composer.json`): outside the corpus, so no vendor file is a corpus file."""
    php = executable(prefix, spec["exe"])
    for project, manifest in spec["composer"].items():
        vendor = prefix / "vendor" / project / "vendor"
        (vendor.parent / "composer.json").parent.mkdir(parents=True, exist_ok=True)
        (vendor.parent / "composer.json").write_text("{}\n", encoding="utf-8")  # tools find a project's vendor/ by its composer.json (PHPStan: composerAutoloaderProjectPaths); an empty one declares no php version
        if (vendor / "autoload.php").exists():
            continue
        work = prefix / "deps" / project
        shutil.rmtree(work, ignore_errors=True)
        shutil.copytree(ROOT / manifest, work)
        env = {**os.environ, "COMPOSER_VENDOR_DIR": str(vendor), "COMPOSER_HOME": str(prefix / "composer-home"), "COMPOSER_NO_INTERACTION": "1"}
        command = "install" if (work / "composer.lock").exists() else "update"
        run([php, prefix / "composer.phar", command, "--no-scripts", "--no-plugins", "--no-interaction", "--ignore-platform-reqs", "--no-progress", *(["--no-audit"] if command == "update" else [])], work, env=env)
        for package in spec.get("composer_require", {}).get(project, []):  # what the project's test runner installs on its own (Composer: simple-phpunit installs PHPUnit 9.6 into vendor/bin/.phpunit, where PHPStan never looks), pinned
            run([php, prefix / "composer.phar", "require", "--dev", package, "--update-with-dependencies", "--no-scripts", "--no-plugins", "--no-interaction", "--ignore-platform-reqs", "--no-progress", "--no-audit"], work, env=env)


def fixtures(config, folder, filters):
    """The fixtures as (name, path, ratchet group or None, its source); names are the keys of differences.yaml.
    `filters` counts the files left out: `skip`, `skip_with` and non-UTF-8 files."""
    out = []
    for source in config["fixtures"]:
        base = ROOT / source["dir"] if "dir" in source else folder
        suffixes = tuple(source["suffixes"])
        if "list" in source:
            listing = base / source["list"]
            if not listing.exists():
                print(f"{source['dir']} not fetched: its files are skipped")
                continue
            paths = [(rel, base / rel) for rel in listing.read_text().splitlines() if rel.startswith(source["starts"])]
        else:
            walk = base.rglob("*") if source.get("recursive") else base.iterdir()
            paths = [(p.relative_to(base).as_posix(), p) for p in sorted(walk) if p.is_file() and ".git" not in p.parts]
        seen = Counter()  # `per_dir`: the first N files by path of each top folder (a project of a corpus)
        for rel, path in paths:
            only = source.get("only", "")
            if not rel.endswith(suffixes) or path.name == "sudsy.yaml" or not rel.startswith(tuple(only) if isinstance(only, list) else only):
                continue
            if "names" in source and not re.search(source["names"], path.name):  # the file names of a language with no extension (`Dockerfile`, `Dockerfile.dev`)
                continue
            if "per_dir" in source:
                seen[rel.split("/")[0]] += 1
                if seen[rel.split("/")[0]] > source["per_dir"]:
                    filters["per_dir"] += 1
                    continue
            if rel.startswith(source.get("skip", "\0")):
                filters["skip"] += 1
                continue
            if "skip_with" in source and path.with_suffix(source["skip_with"]).exists():
                filters["skip_with"] += 1
                continue
            try:
                path.read_bytes().decode("utf-8")
            except UnicodeDecodeError:
                filters["non-UTF-8 files"] += 1
                NON_UTF8.update({source.get("prefix", "") + rel, path.as_posix().split("tests/conformance/corpus/")[-1]})
                continue
            if "generated" in config and generated(config["generated"], path):
                filters["generated"] += 1
                continue
            out.append((source.get("prefix", "") + rel, path, source.get("ratchet"), source))
    return out


def generated(spec, path):
    """A file the tool leaves alone because it documents it as generated code. `spec` is `{"lang": <crate dir>}`: the
    list is the one Sudsy skips by (`generated_names`, `generated_header_markers`, `generated_header_skip` of the
    language's lang.yaml), so the filter and the formatter share one source."""
    options = {}
    for line in (ROOT / "crates" / spec["lang"] / "lang.yaml").read_text(encoding="utf-8").splitlines():
        m = re.match(r"\s*- (generated_\w+): (.*)$", line)
        if m:
            options[m.group(1)] = json.loads(m.group(2))[0]
    stem = path.name.rsplit(".", 1)[0].lower()
    for g in map(str.lower, options["generated_names"]):
        if (g.startswith("*") and stem.endswith(g[1:])) or (g.endswith("*") and stem.startswith(g[:-1])) or stem == g:
            return True
    head, rest = "", path.read_text(encoding="utf-8").lstrip("\ufeff")
    while True:
        rest = rest.lstrip()
        if rest.startswith("//"):
            line, _, rest = rest.partition("\n")
            head += line[2:] + "\n"
        elif rest.startswith("/*"):
            comment, _, rest = rest.partition("*/")
            head += comment[2:] + "\n"
        elif any(rest.startswith(p) for p in options["generated_header_skip"]):
            rest = rest.partition("\n")[2]
        else:
            break
    return any(m.lower() in head.lower() for m in options["generated_header_markers"])


def placed(folder, name, source):
    """Where a fixture is copied: a flat name (`/` as `__`), or with its folders for a source with `nested` (the file's
    place selects the language or dialect: `tsconfig.json`, `.vscode/*.json`)."""
    return folder / (name if source.get("nested") else name.replace("/", "__"))


def copy_path(config, source, here, name):
    """`placed`, with the extension the language is read from: `copy_as` of the source (`.h` of a C++ project is `.cpp`),
    `copy_suffix` of the tool (Angular: `*.component.html`)."""
    copy = placed(here, name, source)
    if "copy_as" in source:
        copy = copy.with_name(copy.name + source["copy_as"])
    if "copy_suffix" in config:
        copy = copy.with_name(copy.name.removesuffix(".html") + config["copy_suffix"])
    return copy


def upstream(config, prefix, version, copy, work, style=None, folder=None):
    """The tool's output for the copy of a fixture, or None when it rejects the file."""
    exe = executable(prefix, config.get("install", {}).get("exe", ""))
    mode = config["input"]
    text = copy.read_bytes().decode("utf-8")
    if mode == "in-place":  # the tool rewrites its file: it gets a copy of the copy
        copy = work / "theirs" / copy.name
        copy.parent.mkdir(exist_ok=True)
        shutil.copy(work / copy.name, copy)
    group = re.search(config["group"], copy.name) if "group" in config else None  # `{group}`: the first group of the fixture name (sqlfluff: its dialect)
    command, env = tool_command(config, prefix, exe=exe, file=(copy.name if mode == "in-place" else copy.relative_to(work).as_posix()), group=group.group(1) if group else "", style=style, folder=folder or ROOT / "tests/parity" / config.get("folder", ""))
    r = run(command, copy.parent if mode == "in-place" else work, text if mode == "stdin" else None, check=False, env=env)
    # `reject_output`: a regex over the output; the tool exits non-zero for other reasons too (sqlfluff: unfixable findings left after a fix), and only a match is a rejection
    if r.returncode != 0 and not ("reject_output" in config and mode == "in-place" and r.returncode == 1 and not re.search(config["reject_output"], (r.stdout + r.stderr).decode("utf-8", "replace"))):
        if config.get("tool_may_reject"):
            return None
        raise SystemExit(f"{' '.join(command)} failed:\n{r.stderr.decode()}{r.stdout.decode()}")
    return copy.read_bytes().decode("utf-8") if mode == "in-place" else r.stdout.decode("utf-8")


def upstream_batch(config, prefix, found, work):
    """`input: batch`: one run of the tool over copies of all fixtures (a JVM tool starts too slowly for one run
    per file). Returns name -> the rewritten text, or None for a file the tool names in `reject` (its output)."""
    exe = executable(prefix, config["install"].get("exe", ""))
    folder = work / "theirs"
    folder.mkdir()
    names = {}
    for name, fixture, _, _ in found:
        copy = name.replace("/", "__")
        shutil.copy(fixture, folder / copy)
        names[name] = copy
    for rel, text in config.get("files", {}).items():  # config files the tool reads from the folder
        (folder / rel).write_text(text.format(prefix=prefix), encoding="utf-8")
    command = [str(a).format(exe=exe, prefix=prefix, root=ROOT) for a in config["command"]]
    rejected = set()
    if config.get("cache"):  # a slow tool: its output is kept per version, command and fixture contents (RuboCop over the whole Ruby corpus takes minutes)
        key = hashlib.sha256(json.dumps([command, config.get("args"), config.get("files"), prefix.name, sorted((n, hashlib.sha256((folder / c).read_bytes()).hexdigest()) for n, c in names.items())]).encode()).hexdigest()[:16]
        kept = ROOT / ".cargo/tmp/rb" / f"{config['catalog']}-batch-{key}"  # the slot may be wiped: the cache lives in the worktree, only the files the tool changed (the others are the input)
        if kept.is_dir():
            return {n: (kept / c if (kept / c).exists() else folder / c).read_bytes().decode("utf-8") for n, c in names.items()}

    def reject(r):
        if "reject" in config:
            rejected.update(re.findall(config["reject"], (r.stdout + r.stderr).decode("utf-8", "replace"), re.M))

    if "chunk" in config:  # a tool that stops at the first file that raises: runs of `chunk` files, a failed one file by file
        copies = list(names.values())
        for i in range(0, len(copies), config["chunk"]):
            group = copies[i : i + config["chunk"]]
            r = run(command + group, folder, check=False)
            if r.returncode == 0:
                reject(r)
                continue
            for copy in group:
                r = run(command + [copy], folder, check=False)
                reject(r)
                if r.returncode != 0:
                    rejected.add(copy)
    else:
        reject(run(command + (config.get("args") or sorted({"*" + Path(c).suffix for c in names.values()})), folder, check=False))
    if config.get("cache"):
        kept.mkdir(parents=True)
        for name, fixture, _, _ in found:
            if (folder / names[name]).read_bytes() != fixture.read_bytes():
                shutil.copy(folder / names[name], kept / names[name])
    return {n: None if c in rejected else (folder / c).read_bytes().decode("utf-8") for n, c in names.items()}


def regression_counts(found, results):
    """Per group, the files the tool leaves alone that Sudsy changes: a formatter that is right today may only get better."""
    changed = Counter()
    for (_, path, group, _), (theirs, ours, _, _) in zip(found, results):
        changed[group or "fixtures"] += theirs is not None and ours is not None and theirs == path.read_bytes().decode("utf-8") != ours
    return changed


def check_reference(spec, parity, known, rejected):
    """The reference parser of a language (docs/ai/design/parity-target.md: `invalid-input` for a file Sudsy rejects needs the
    language's own parser to reject it too): `spec` is `{npm, version, script}`, a Node script that exits 1 on a syntax error.
    A file it accepts is put in `known.refuted`, which `Differences.status` reports as REVIEW (counts against Sudsy)."""
    prefix = parity / f"reference-{spec['npm']}-{spec['version']}"
    install({"npm": spec["npm"]}, spec["npm"], spec["version"], prefix)
    for name, path in rejected:
        if name in known.files and any(known.intentional.get(n) == "invalid-input" for n in known.files[name]):
            r = run(["node", "-e", spec["script"], prefix / "node_modules" / spec["npm"], path], ROOT, check=False)
            if r.returncode == 0:
                known.refuted.add(name)
                print(f"{name}: Sudsy rejects it, the reference parser ({spec['npm']} {spec['version']}) accepts it: a Sudsy bug")


def sudsy_format(sudsy, fixture, work, command="format"):
    r = run([sudsy, command, fixture.relative_to(work).as_posix()], work, check=False)
    # None: Sudsy rejects the file; `refused`: its safety check did (an internal error, not a syntax error)
    return (fixture.read_bytes().decode("utf-8") if r.returncode == 0 else None), b"internal error" in r.stderr


INTENTIONAL = ("invalid-input", "tool-wrong")


def pin(text):
    """The pin of Sudsy's output for a `tool-wrong` file."""
    return hashlib.sha256(text.encode("utf-8")).hexdigest()[:16]


def percent(part, whole):
    return f"{round(1000 * part / whole) / 10 if whole else 100.0}%"


class Differences:
    """`tests/parity/<tool>/differences.yaml` (docs/ai/development/parity.md): `differences` (name -> `<choice|gap>  <text>`),
    `files` (fixture key -> names; a ratchet group's name stands for all its files), `ratchet` (group -> least identical files),
    `intentional` (name -> `invalid-input` or `tool-wrong`) and `pins` (file -> `pin()` of Sudsy's output, for the `tool-wrong` names).
    `lint`: the keys with a space are the lint mode's (a tool's two modes share the file); a lint finding is pinned by its key (file, line, rule).
    The target number leaves out the intentional files: `status` says which, `tally` counts, `summary` prints the target."""

    def __init__(self, folder, lint=False):
        path = folder / "differences.yaml"
        data = (yaml.safe_load(path.read_text(encoding="utf-8")) if path.exists() else None) or {}
        self.defs = {n: v.split(None, 1) for n, v in (data.get("differences") or {}).items()}
        self.files, self.listed = {}, {}  # `files` names (a name may end in `:N`: N files of the group have it as their first cause) and per key the files so attributed
        for k, v in (data.get("files") or {}).items():
            names = [n.strip() for n in v.split(",")]
            self.files[k] = [n.partition(":")[0] for n in names]
            self.listed[k] = sum(int(n.partition(":")[2] or 0) for n in names)
        self.fixes = data.get("fixes") or {}  # gap name or lint rule -> where and what to change (the queue, docs/ai/parity/queue.md)
        self.missed = data.get("missed") or {}  # lint rule -> findings of the corpus it misses (the coverage run checks it)
        self.unclassified = data.get("unclassified")  # most differing files of the ratchet groups that no `name:N` of `files` explains
        self.ratchet = data.get("ratchet") or {}
        self.target_files = data.get("target-files") or {}  # group -> files of the target (all minus the intentional ones, tool-rejected included): the README table's denominator
        self.by_group = {}  # group -> [files of the target, files measured]
        self.second_format = data.get("second-format") or {}  # group -> most files that may change on a second format
        self.regressions = data.get("regressions") or {}  # group -> most files the tool leaves as they are that Sudsy changes
        self.refused = data.get("refused") or {}  # group -> most files Sudsy's safety check refuses (a lower bound of the second-format count)
        self.intentional = data.get("intentional") or {}
        self.pins = data.get("pins") or {}
        self.not_comparable = data.get("not-comparable") or {}  # lint coverage: rule -> the evidence that its findings need what the corpus lacks (left out of the target, counted in every run)
        self.flags = data.get("flags") or {}  # reference-tool flag that turns a feature off -> why it is there (a named gap, printed with its count)
        self.refuted = set()  # `invalid-input` files that the reference parser accepts: Sudsy's rejection is a bug
        self.lint = lint
        self.differing = {}  # key -> files that differ, listed or not
        self.same = self.total = 0
        self.groups = Counter()  # intentional kind or `REVIEW` -> files

    def known(self, key, count=1):
        """Records that `key` differs; True when it is listed."""
        self.differing[key] = count
        return key in self.files

    def status(self, key, ours=None):
        """`invalid-input` or `tool-wrong` for a listed key whose name is intentional; `REVIEW` for a `tool-wrong` file whose
        Sudsy output is not the pinned one (it counts against Sudsy); else None."""
        if key in self.refuted:
            return "REVIEW"
        kinds = {self.intentional[n] for n in self.files.get(key, []) if n in self.intentional}
        if "invalid-input" in kinds:
            return "invalid-input"
        if "tool-wrong" in kinds:
            return "tool-wrong" if self.lint or (ours is not None and self.pins.get(key) == pin(ours)) else "REVIEW"
        return None

    def tally(self, same, status=None, count=1, group="fixtures"):
        self.total += count
        row = self.by_group.setdefault(group, [0, 0])
        row[0] += count * (same or status not in ("invalid-input", "tool-wrong"))
        row[1] += count
        self.same += count if same else 0
        if status and not same:
            self.groups[status] += count

    def summary(self):
        """The target: identical of all but the intentional ones (`REVIEW` stays in)."""
        left = self.total - sum(n for k, n in self.groups.items() if k != "REVIEW")
        groups = ", ".join(f"{k} {self.groups[k]}" for k in (*INTENTIONAL, "REVIEW"))
        print(f"target {percent(self.same, left)} ({self.same} of {left}); intentional: {groups}")

    def finish(self, filtered=False, scope=None):
        """Prints the problems of the lists and the report; returns the number of problems. `scope`: the files this lint run covered
        (a tool's fixture run and its coverage run share the file; a key of the other run's files is not stale here)."""
        bad = 0
        used = {n for names in self.files.values() for n in names}
        for n in sorted(used - set(self.defs)):
            print(f"UNDEFINED difference name: {n}")
            bad += 1
        for n in sorted(set(self.defs) - used):
            print(f"UNUSED difference name: {n}")
            bad += 1
        for n in sorted(set(self.fixes) - set(self.defs) - set(self.missed)):
            print(f"STALE fixes note: {n} is neither a difference name nor a rule of `missed`")
            bad += 1
        for n, kind in self.intentional.items():
            if n not in self.defs or kind not in INTENTIONAL:
                print(f"intentional {n}: a defined name with the value {' or '.join(INTENTIONAL)}, not `{kind}`")
                bad += 1
        for n, (kind, _) in self.defs.items():
            if kind not in ("choice", "gap"):
                print(f"difference {n}: the kind is `choice` or `gap`, not `{kind}`")
                bad += 1
        if not filtered:
            for key in self.files:
                if (" " in key) == self.lint and key not in self.differing and key not in NON_UTF8 and (scope is None or key.split(" ")[0] in scope):
                    print(f"STALE: {key} is listed but no longer differs")
                    bad += 1
        counts = Counter()
        for key, count in self.differing.items():
            for n in self.files.get(key, []):
                counts[n] += count
        for kind in ("choice", "gap"):
            row = ", ".join(f"{n} {counts[n]}" for n, (k, _) in self.defs.items() if k == kind and counts[n])
            print(f"{kind}: {row or 'none'}")
        return bad


def notebook_cells(text):
    """What a notebook comparison keeps (`compare: notebook-cells`): the type and the source of each cell. The tool writes
    the JSON with its own layout (indent, key order, escapes, ids, dropped null fields); Sudsy keeps every byte outside
    the code cells as written, which its safety check guarantees."""
    try:
        cells = json.loads(text).get("cells")
    except (ValueError, AttributeError):
        return text
    if cells is None:
        return text
    return json.dumps([[c.get("cell_type"), "".join(c["source"]) if isinstance(c.get("source"), list) else c.get("source")] for c in cells])


def verdict(name, theirs, ours, diffs, label, quiet=False, group="fixtures"):
    """Prints the result of one fixture; returns 1 for a mismatch. `quiet`: no line for an equal pair, no diff."""
    if theirs is not None and theirs == ours:
        diffs.tally(True, group=group)
        if not quiet:
            print(f"{name}: ok")
        return 0
    if theirs is None:  # the tool rejecting a file is the evidence it is invalid input: the target leaves it out
        diffs.tally(False, "invalid-input", group=group)
        if not quiet:
            print(f"{name}: rejected by the tool, intentional: invalid-input")
        return 0
    known = diffs.known(name)
    state = diffs.status(name, ours) if known else None
    diffs.tally(False, state, group=group)
    if state == "REVIEW":
        print(f"{name}: REVIEW (Sudsy's output differs from the pinned one; counts against Sudsy)")
        return 1
    if known:
        print(f"{name}: known difference ({', '.join(diffs.files[name])})" + (f", intentional: {state}" if state else ""))
        return 0
    print(f"{name}: DIFFERENT (the tool rejects the file)" if theirs is None else f"{name}: DIFFERENT")
    if theirs is not None and not quiet:
        sys.stdout.writelines(difflib.unified_diff(theirs.splitlines(True), (ours or "").splitlines(True), label, "sudsy"))
    return 1


def editorconfig_cases(version, sudsy, parity):
    """One case per `<case>.editorconfig` of tests/parity/dotnet-format: `<case>.cs`, else the
    shared `input.cs`, formatted by `dotnet format whitespace` and by Sudsy in a folder that holds the
    `.editorconfig` and the folder's sudsy.yaml."""
    folder = ROOT / "tests/parity/dotnet-format"
    known = Differences(folder)
    cases = sorted(p.stem for p in folder.glob("*.editorconfig"))
    bad = 0
    for name in cases:
        outputs = []
        for side in ("dotnet-format", "sudsy"):
            case = parity / f"dotnet-format-work/{side}/{name}"
            shutil.rmtree(case, ignore_errors=True)
            case.mkdir(parents=True)
            shutil.copy(folder / f"{name}.editorconfig", case / ".editorconfig")
            own = folder / f"{name}.cs"
            shutil.copy(own if own.exists() else folder / "input.cs", case / "input.cs")
            shutil.copy(folder / "sudsy.yaml", case / "sudsy.yaml")
            if side == "sudsy":
                outputs.append(sudsy_format(sudsy, case / "input.cs", case)[0])
            else:
                run(["dotnet", "format", "whitespace", case, "--folder"], case)
                outputs.append((case / "input.cs").read_bytes().decode("utf-8"))
        bad += verdict(name, *outputs, known, "dotnet-format")
    bad += known.finish()
    known.summary()
    print(f"{len(cases)} cases, {bad} mismatch(es) (dotnet format {version})")
    return bad


def use_worker(config):
    """Builds the C# worker for an entry that needs it and points Sudsy at it."""
    if "worker" in config.get("needs", []):
        run(["dotnet", "build", "--nologo", "-v:q", "workers/csharp/Sudsy.Worker"], ROOT)
        os.environ.setdefault("SUDSY_CSHARP_WORKER", str(ROOT / "workers/csharp/Sudsy.Worker/bin/Debug/net10.0/Sudsy.Worker.dll"))


def prepare(config, parity):
    """Builds what the entry needs, fetches its corpora and installs the tool; returns (version, prefix)."""
    use_worker(config)
    for script in config.get("fetch", []):
        run(["bash", script], ROOT)
    if "catalog" not in config:
        return run(config["version_command"], ROOT).stdout.decode().strip(), None
    version = config.get("version") or catalog_version(config["catalog"])  # `version`: the tool's release when the catalog pins something else (tflint: its ruleset)
    prefix = parity / f"{config['catalog']}-{version}"
    spec = config.get("install")
    if spec:  # a name: the install of that format entry
        install(tools()["format"][spec]["install"] if isinstance(spec, str) else spec, config["catalog"], version, prefix)
    return version, prefix


def corpus_list(path, key):
    """The `- item` lines under `key:` of a YAML file of the corpus, quotes stripped."""
    lines = path.read_text(encoding="utf-8").splitlines()
    start = next((i for i, line in enumerate(lines) if line.startswith(f"{key}:")), None)
    if start is None:
        raise SystemExit(f"{path} has no `{key}:` list")
    items = []
    for line in lines[start + 1 :]:
        m = re.match(r"\s+-\s+(.*?)\s*$", line)
        if not m:
            break
        items.append(m.group(1).strip("'\""))
    return items


def style_files(config, folder, work):
    """A copy of the style files of `folder` in `work`, where a fixture source with `style_list`
    (`file`, `key`, `option`) adds a list from a file of its corpus to the tool's style `<style>.clang-format`
    (as `key:`) and to Sudsy's `<style>.yaml` (as `option`, under the first `format:` section). A missing
    corpus file stops the run."""
    out = work / "styles"
    shutil.copytree(folder, out)
    for source in config["fixtures"]:
        add = source.get("style_list")
        if not add or "style" not in source:
            continue
        path = ROOT / source["dir"] / add["file"]
        if not path.is_file():
            raise SystemExit(f"{path} is missing: run {' '.join(config.get('fetch', []))} (its `{add['key']}` goes into the {source['style']} style)")
        items = corpus_list(path, add["key"])
        tool = out / f"{source['style']}.clang-format"
        tool.write_text(tool.read_text(encoding="utf-8") + f"{add['key']}:\n" + "".join(f"  - '{i}'\n" for i in items), encoding="utf-8")
        ours = out / f"{source['style']}.yaml"
        lines = ours.read_text(encoding="utf-8").splitlines(keepends=True)
        at = next(i for i, line in enumerate(lines) if line.strip() == "format:") + 1
        lines[at:at] = [f"    {add['option']}:\n", *(f"      - {i}\n" for i in items)]
        ours.write_text("".join(lines), encoding="utf-8")
    return out


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    tool, sudsy, target = sys.argv[1:4]
    config = tools()["format"][tool]
    parity = Path(target).resolve() / "parity"
    version, prefix = prepare(config, parity)
    if config.get("kind") == "editorconfig-cases":
        return 1 if editorconfig_cases(version, sudsy, parity) else 0
    folder = ROOT / "tests/parity" / tool
    work = parity / f"{tool}-work"
    shutil.rmtree(work, ignore_errors=True)
    work.mkdir(parents=True)
    shutil.copy(folder / "sudsy.yaml", work / "sudsy.yaml")
    config["folder"] = tool
    known = Differences(folder)
    styles = style_files(config, folder, work)
    only = os.environ.get("PARITY_FILTER")  # a name substring: those fixtures only, with their diffs, no ratchet
    filters = Counter()
    found = [f for f in fixtures(config, folder, filters) if only is None or only in f[0]]
    quiet = bool(config.get("quiet")) and only is None

    for style in {f[3].get("style") for f in found if f[3].get("style")}:  # set up before the threads read it
        (work / f"style-{style}").mkdir()
        shutil.copy(styles / f"{style}.yaml", work / f"style-{style}" / "sudsy.yaml")

    batch = upstream_batch(config, prefix, found, work) if config["input"] == "batch" else None

    flagged = Counter()  # flag -> files the flag changed the result of
    wrapped = {}  # `wrap_compare`: fixture -> its token-and-gap result (scripts/parity_wrap.py)
    controlled = bool(os.environ.get("PARITY_CONTROLS"))
    checker = None
    if "wrap_compare" in config:
        checker = parity_wrap.Checker(config["wrap_compare"], ROOT, [os.environ.get("PARITY_DUMP", sudsy), *config["wrap_compare"]["dump"]], work / "dump-tmp")
        if controlled:
            parity_wrap.write_variants(config["wrap_compare"], styles, work / "variants", {f[3].get("style") for f in found if f[3].get("style")})

    def compare(item):
        name, fixture, _, source = item
        style = source.get("style")
        here = work / f"style-{style}" if style else work  # a style has its own folder and `sudsy.yaml` (`<style>.yaml` of the tool's folder)
        copy = copy_path(config, source, here, name)
        copy.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy(fixture, copy)
        theirs = batch[name] if batch is not None else upstream(config, prefix, version, copy, here, style, styles)
        ours, refused = sudsy_format(sudsy, copy, here, config.get("sudsy_command", "format"))
        # Sudsy's own output must be a fixed point; the files that change on a second format are counted.
        again = None if ours is None else sudsy_format(sudsy, copy, here, config.get("sudsy_command", "format"))[0]
        unstable = again is not None and again != ours
        masked = False
        if config.get("ignore_trailing_blanks"):  # the tool keeps the trailing blanks of a line, Sudsy never does
            before = theirs == ours
            theirs, ours = (parity_landing.normalize(config, t) if t is not None else None for t in (theirs, ours))
            masked = not before and theirs == ours
        flagged["ignore_trailing_blanks"] += masked
        if config.get("compare") == "notebook-cells":
            theirs, ours = (notebook_cells(t) if t is not None else None for t in (theirs, ours))
        if checker and theirs is not None:
            wrapped[name] = parity_wrap.file_result(checker, config["wrap_compare"], styles, style, theirs, ours, copy, controlled, lambda variant: upstream(config, prefix, version, copy, here, f"{style}~{variant}", work / "variants"))
        return theirs, ours, unstable, refused

    with ThreadPoolExecutor(os.cpu_count() if config.get("parallel") else 1) as pool:
        results = list(pool.map(compare, found))
    if checker:  # a file with equal tokens and gaps counts as equal
        parity_wrap.report(found, results, wrapped, target, os.environ.get("PARITY_LABEL", "run"), config["wrap_compare"]["note"])
        results = [(t, t if (w := wrapped.get(f[0])) and w["ok"] else o, u, r) for f, (t, o, u, r) in zip(found, results)]
    bad, groups = 0, {}  # a ratchet group is counted: [identical, different, rejected by the tool]
    unstable = Counter()  # group (or "fixtures") -> files that change on a second format
    refusals = Counter()  # group (or "fixtures") -> files Sudsy's safety check refuses
    if "reference_parser" in config and only is None:  # Sudsy rejects a file the reference parser accepts: a Sudsy bug, not invalid input
        check_reference(config["reference_parser"], parity, known, [(n, placed(work, n, src)) for (n, _, _, src), (_, o, _, _) in zip(found, results) if o is None])
    for (name, path, group, _), (_, _, again, refused) in zip(found, results):
        if again:
            unstable[group or "fixtures"] += 1
            if only is not None:
                print(f"{name}: changes on a second format")
        if refused:
            refusals[group or "fixtures"] += 1
            if only is not None:
                print(f"{name}: refused by the safety check")
    for (name, _, group, _), (theirs, ours, _, _) in zip(found, results):
        if group and only is None:
            tally = groups.setdefault(group, [0, 0, 0])
            same = theirs is not None and theirs == ours
            tally[2 if theirs is None else 0 if same else 1] += 1
            state = "invalid-input" if theirs is None else None if same or not known.known(name) else known.status(name, ours)
            known.tally(same, state, group=group)
            if state == "REVIEW":
                print(f"{name}: REVIEW (Sudsy's output differs from the pinned one; counts against Sudsy)")
                bad += 1
        else:
            bad += verdict(name, theirs, ours, known, tool, quiet, group or "fixtures")
            if only is None:  # a run without ratchet groups is one group, `fixtures`
                tally = groups.setdefault("fixtures", [0, 0, 0])
                tally[2 if theirs is None else 0 if theirs == ours else 1] += 1
    for group, (same, different, rejected) in sorted(groups.items()):
        floor = known.ratchet.get(group, 0)
        if different:
            known.known(group, different)
        note = "BELOW the minimum" if same < floor else f"raise the minimum in differences.yaml to {same}" if same > floor else "at the minimum"
        print(f"ratchet {group}: {same} identical, {different} different, {rejected} rejected by {tool} ({note}: {floor})")
        bad += same < floor
    if only is None:
        for group, (target, measured) in sorted(known.by_group.items()):  # the denominator of the README table: a stale value fails like a ratchet
            top = known.target_files.get(group)
            note = "at the recorded value" if top == target else f"STALE: set target-files {group} in differences.yaml to {target}"
            print(f"target-files {group}: {target} of {measured} files ({note}: {top})")
            bad += top != target
        diff = {g: max(0, t - groups[g][0]) for g, (t, _) in known.by_group.items() if g in groups and g != "fixtures"}
        now, top, left = sum(max(0, d - known.listed.get(g, 0)) for g, d in diff.items()), known.unclassified, sum(diff.values())
        note = "no `unclassified:` yet: set it to this count" if top is None else "ABOVE the maximum" if now > top else f"lower `unclassified:` in differences.yaml to {now}" if now < top else "at the maximum"
        print(f"unclassified: {now} of {left} differing files of the ratchet groups ({round(100 * now / left) if left else 0}%; {note}{'' if top is None else f': {top}'}); name:N in `files` classifies")
        bad += top is not None and now > top
        ceilings = known.second_format
        for group in sorted(set(unstable) | set(ceilings)):
            now, top = unstable[group], ceilings.get(group, 0)
            note = "ABOVE the maximum" if now > top else f"lower the maximum in differences.yaml to {now}" if now < top else "at the maximum"
            print(f"second format {group}: {now} files change ({note}: {top})")
            bad += now > top
        changed = regression_counts(found, results)
        for group in sorted(known.regressions):
            now, top = changed[group], known.regressions[group]
            note = "ABOVE the maximum" if now > top else f"lower the maximum in differences.yaml to {now}" if now < top else "at the maximum"
            print(f"regressions {group}: {now} files the tool leaves alone change ({note}: {top})")
            bad += now > top
        # (refused files are not formatted, so the second-format count above leaves them out)
        for group in sorted(set(refusals) | set(known.refused)):
            now, top = refusals[group], known.refused.get(group, 0)
            note = "ABOVE the maximum" if now > top else f"lower the maximum in differences.yaml to {now}" if now < top else "at the maximum"
            print(f"refused {group}: {now} files ({note}: {top})")
            bad += now > top
    bad += known.finish(only is not None)
    known.summary()
    if os.environ.get("PARITY_WRITE") and only is None:  # the landing sample (scripts/parity_landing.py): the corpus files that equal the tool's output now
        if "wrap_compare" in config:
            print("landing sample: not written (the comparison is not byte equality)")
        else:
            parity_landing.write(folder, tool, {path: theirs for (_, path, _, _), (theirs, ours, _, _) in zip(found, results) if theirs is not None and theirs == ours})
    rejected = sum(theirs is None for theirs, *_ in results)
    for flag, why in known.flags.items():  # reference-tool flags that turn a feature off: named gaps, with the files they apply to
        count = filters["skip_with"] if flag == "skip_with" else flagged[flag] if flag in flagged else len(found)
        print(f"flag {flag} (gap): {count} fixtures; {why}")
    print("input filters: " + ", ".join(f"{k} {filters[k]}" for k in ("non-UTF-8 files", "skip", "skip_with", "per_dir", "generated") if k != "generated" or "generated" in config) + f", rejected by {tool} {rejected}")
    if "--pin" in sys.argv:  # for the lead to paste into `pins:` after review: the runner never writes them
        print("pins (tool-wrong names):")
        for (name, *_), (_, ours, *_) in zip(found, results):
            wrong = any(known.intentional.get(n) == "tool-wrong" for n in known.files.get(name, []))
            if ours is not None and (wrong or known.status(name, ours) == "REVIEW"):  # every file the run printed as REVIEW has its current hash here
                print(f"  {name}: {pin(ours)}" + ("" if wrong else "  (REVIEW)"))
    print(f"{len(found)} fixtures, {bad} mismatch(es) ({tool} {version})")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
