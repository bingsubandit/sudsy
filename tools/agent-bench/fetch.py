"""fetch.py [REPO...]: clone the pinned repositories (blobless, bare) into cache/ and build their tool
environments (a Python venv per repo from `python`, node_modules from the pin's lockfile). Idempotent."""
import shutil
import subprocess
import sys

from common import CACHE, bare, git, repos, run_env, sh


def clone(name, cfg):
    b = bare(name)
    if not b.exists():
        CACHE.mkdir(parents=True, exist_ok=True)
        subprocess.run(["git", "clone", "-q", "--bare", "--filter=blob:none", cfg["url"], str(b)], check=True)
    if subprocess.run(["git", "cat-file", "-e", cfg["pin"] + "^{commit}"], cwd=b).returncode != 0:
        git(b, "fetch", "-q", "origin", cfg["pin"])


def export(name, rev, dest):
    """Write the tree of `rev` into `dest` (no .git)."""
    dest.mkdir(parents=True, exist_ok=True)
    tar = subprocess.Popen(["git", "archive", rev], cwd=bare(name), stdout=subprocess.PIPE)
    subprocess.run(["tar", "-x", "-C", str(dest)], stdin=tar.stdout, check=True)
    if tar.wait() != 0:
        raise RuntimeError(f"git archive {rev} failed for {name}")


def python_env(name, cfg):
    venv = CACHE / "venv" / name
    stamp = venv / "bench-reqs.txt"
    want = "\n".join(cfg["python"]) + "\n"
    if stamp.exists() and stamp.read_text() == want:
        return
    shutil.rmtree(venv, ignore_errors=True)
    subprocess.run(["uv", "venv", "-q", "--python", "3.13", str(venv)], check=True)
    subprocess.run(["uv", "pip", "install", "-q", "--python", str(venv / "bin" / "python"), *cfg["python"]], check=True)
    stamp.write_text(want)


def node_env(name, cfg):
    root = CACHE / "node" / name
    if (root / "node_modules").exists():
        return
    shutil.rmtree(root, ignore_errors=True)
    export(name, cfg["pin"], root)
    r = sh(cfg["node"], cwd=root, env=run_env({"name": name}), timeout=1200)
    if r.returncode != 0:
        raise RuntimeError(f"{name}: {cfg['node']} failed:\n{r.stdout[-2000:]}\n{r.stderr[-2000:]}")


def main(names):
    all_repos = repos()["repo"]
    for name in names or all_repos:
        cfg = {**all_repos[name], "name": name}
        clone(name, cfg)
        if "python" in cfg:
            python_env(name, cfg)
        if "node" in cfg:
            node_env(name, cfg)
        print(f"{name}: ready at {cfg['pin'][:12]}")


if __name__ == "__main__":
    main(sys.argv[1:])
