"""Shared helpers of the agent bench: paths, config files, shell calls."""

import fnmatch
import json
import os
import pathlib
import shlex
import subprocess
import tomllib

HERE = pathlib.Path(__file__).resolve().parent
REPO_ROOT = HERE.parent.parent
CACHE = HERE / "cache"  # gitignored: bare clones, tool environments, node_modules
TASKS = HERE / "tasks"


def work_root():
    """Where run directories go: outside every git checkout and every directory with a CLAUDE.md,
    so the subject reads none of Sudsy's own instructions. AGENT_BENCH_WORK, else the build slot."""
    if os.environ.get("AGENT_BENCH_WORK"):
        return pathlib.Path(os.environ["AGENT_BENCH_WORK"])
    slot = REPO_ROOT / ".cargo" / "tmp" / "SLOT"
    if slot.exists():
        return pathlib.Path(slot.read_text().strip()) / "agent-bench"
    return pathlib.Path.home() / ".cache" / "sudsy-agent-bench"


def load_toml(path):
    return tomllib.loads(pathlib.Path(path).read_text())


def lock():
    return load_toml(HERE / "bench.lock.toml")


def repos():
    return load_toml(HERE / "repos.toml")


def task_dir(version, task_id):
    return TASKS / version / task_id


def load_task(version, task_id):
    t = load_toml(task_dir(version, task_id) / "task.toml")
    t["id"] = task_id
    t["version"] = version
    return t


def task_ids(version):
    return sorted(
        p.name for p in (TASKS / version).iterdir() if (p / "task.toml").exists()
    )


def sh(cmd, cwd, env=None, timeout=600, stdin=None):
    """Run a shell command line; never raises on a non-zero exit or a timeout."""
    try:
        return subprocess.run(
            cmd,
            shell=isinstance(cmd, str),
            cwd=cwd,
            env=env,
            capture_output=True,
            text=True,
            timeout=timeout,
            input=stdin,
        )
    except subprocess.TimeoutExpired as e:
        return subprocess.CompletedProcess(
            cmd, 124, e.stdout or "", (e.stderr or "") + "\ntimeout"
        )


def git(cwd, *args, check=True):
    r = subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True)
    if check and r.returncode != 0:
        raise RuntimeError(f"git {' '.join(args)} in {cwd}: {r.stderr.strip()}")
    return r.stdout


def bare(repo_name):
    return CACHE / f"{repo_name}.git"


def matches(path, globs):
    return any(fnmatch.fnmatch(path, g) for g in globs)


def run_env(repo_cfg, extra_path=()):
    """The environment of the subject and of the checks: the repo's tool environment on PATH."""
    env = dict(os.environ)
    paths = [str(p) for p in extra_path]
    venv = CACHE / "venv" / repo_cfg["name"] / "bin"
    if venv.exists():
        paths.append(str(venv))
    paths += [str(CACHE / "gocache" / "bin")]
    env["PATH"] = os.pathsep.join(paths + [env.get("PATH", "")])
    env["GOCACHE"] = str(CACHE / "gocache" / "build")
    env["GOMODCACHE"] = str(CACHE / "gocache" / "mod")
    env["GOFLAGS"] = "-mod=mod"
    env["PYTHONDONTWRITEBYTECODE"] = "1"
    env.pop("VIRTUAL_ENV", None)
    for k, v in (repo_cfg.get("env") or {}).items():
        env[k] = v
    return env


def changed_files(run):
    """Files changed since the start commit, tracked and untracked."""
    out = git(run, "diff", "--name-only", "--no-renames", "bench-base") + git(
        run, "ls-files", "--others", "--exclude-standard"
    )
    return sorted(
        {p for p in out.splitlines() if p and not p.startswith("node_modules/")}
    )


def fix_changed(fix_cmds, exts, run, env=None):
    """The family's fix commands on the changed files only (the stop hook; the reference check): a
    trailing ` .` becomes the file list, `npm run` scripts run as they are, others get the files appended."""
    files = [
        f
        for f in changed_files(run)
        if pathlib.Path(f).suffix in exts and (pathlib.Path(run) / f).exists()
    ]
    if not files:
        return
    args = " ".join(shlex.quote(f) for f in files)
    for c in fix_cmds:
        cmd = (
            c
            if c.startswith("npm run")
            else (c[:-2] if c.endswith(" .") else c) + " " + args
        )
        sh(cmd, cwd=run, env=env, timeout=300)


def write_json(path, obj):
    with open(path, "w", newline="\n") as f:
        json.dump(obj, f, indent=1)
        f.write("\n")
