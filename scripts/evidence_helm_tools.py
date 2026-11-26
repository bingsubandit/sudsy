"""Tool evidence of the Helm leg (docs/ai/design/helm.md): helm lint and helm template with kubeconform on the batch 1 charts,
and prettier on the template files (the formatter that chart repositories run most).

Usage: python3 -I scripts/evidence_helm_tools.py <work-dir>   (the work dir is emptied and filled)
Needs, on PATH or in the environment: HELM (helm 3.16.4), KUBECONFORM (kubeconform 0.6.7), PRETTIER (prettier 3.4.2).

Charts: every folder of the corpus with a Chart.yaml (batch 1 has the chart folders; batch 2 files are single blobs, no chart).
`helm lint` runs on the chart folder; `helm template` renders it with its default values, and kubeconform checks the rendered
documents (Kubernetes 1.31 schemas, missing schemas ignored: CRDs have none). Prettier: each template file is formatted
with its own parser (yaml) on stdin: unchanged (output equals input), changed, or parse error (prettier exits non-zero).
"""
import os
import shutil
import subprocess
import sys
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import evidence  # noqa: E402
import evidence_helm  # noqa: E402

HELM = os.environ.get("HELM", "helm")
KUBECONFORM = os.environ.get("KUBECONFORM", "kubeconform")
PRETTIER = os.environ.get("PRETTIER", "prettier")


def run(args, cwd, env=None, stdin=None, timeout=300):
    e = dict(os.environ, HOME=str(cwd / "home"), XDG_CACHE_HOME=str(cwd / "home"), XDG_CONFIG_HOME=str(cwd / "home"))
    e.update(env or {})
    try:
        return subprocess.run(args, cwd=cwd, env=e, input=stdin, capture_output=True, timeout=timeout, check=False)
    except subprocess.TimeoutExpired:
        return None


def charts(work):
    """The chart folders of the corpus, copied to the work dir (whole folders: lint needs Chart.yaml and values.yaml).
    A subchart (a `charts/<name>` folder of a chart) is left out: its parent is linted with it."""
    root = evidence.CORPUS / "helm"
    out = []
    for c in sorted(root.glob("*/**/Chart.yaml")):
        if c.parent.parent.name == "charts" and (c.parent.parent.parent / "Chart.yaml").is_file():
            continue
        out.append(c.parent)
    dst = work / "charts"
    shutil.rmtree(dst, ignore_errors=True)
    dst.mkdir(parents=True)
    res = []
    for i, c in enumerate(out):
        d = dst / f"{i:03d}"
        shutil.copytree(c, d)
        res.append((c.relative_to(root), d))
    return res


def lint_and_render(work, chart):
    rel, d = chart
    lint = run([HELM, "lint", "--quiet", str(d)], work)
    errs = 0
    text = (lint.stdout + lint.stderr).decode("utf-8", "replace") if lint else ""
    errs = text.count("[ERROR]")
    warns = text.count("[WARNING]")
    rendered = run([HELM, "template", "t", str(d)], work)
    if rendered is None or rendered.returncode:
        msg = (rendered.stderr.decode("utf-8", "replace").strip().splitlines() or [""])[0] if rendered else "timeout"
        return str(rel), (lint.returncode if lint else -1), errs, warns, "render error", msg
    ks = run([KUBECONFORM, "-summary", "-ignore-missing-schemas", "-kubernetes-version", "1.31.0", "-"], work, stdin=rendered.stdout)
    summ = (ks.stdout + ks.stderr).decode("utf-8", "replace") if ks else ""
    summary = next((l for l in summ.splitlines() if l.startswith("Summary")), summ.strip()[-200:])
    return str(rel), (lint.returncode if lint else -1), errs, warns, "rendered", summary


def prettier(f):
    """unchanged, changed or parse error, with prettier's yaml parser on stdin."""
    text = f.read_bytes()
    r = subprocess.run([PRETTIER, "--stdin-filepath", "x.yaml"], input=text, capture_output=True, check=False, timeout=120)
    if r.returncode:
        return "parse error"
    return "unchanged" if r.stdout == text else "changed"


def main():
    work = Path(sys.argv[1])
    work.mkdir(parents=True, exist_ok=True)
    (work / "home").mkdir(exist_ok=True)
    cs = charts(work)
    print(f"chart folders: {len(cs)}")
    with ThreadPoolExecutor(4) as ex:
        rows = list(ex.map(lambda c: lint_and_render(work, c), cs))
    lint_ok = sum(1 for r in rows if r[1] == 0)
    rendered = sum(1 for r in rows if r[4] == "rendered")
    print(f"helm lint exit 0: {lint_ok} of {len(rows)}; [ERROR] lines {sum(r[2] for r in rows)}, [WARNING] lines {sum(r[3] for r in rows)}")
    print(f"helm template renders: {rendered} of {len(rows)}")
    for r in rows:
        print(f"  {r[0]}: lint exit {r[1]}, errors {r[2]}, warnings {r[3]}; {r[4]}: {r[5]}")
    # every template file of the corpus (no per-project cap): the files the evidence reads, before its cap
    root = evidence.CORPUS / "helm"
    files = [f for f in root.rglob("*") if f.is_file() and evidence_helm.DIRS[0][1].match(f.name) and evidence_helm.KEEP(f.relative_to(root))]
    with ThreadPoolExecutor(8) as ex:
        res = list(ex.map(prettier, files))
    c = Counter(res)
    print(f"prettier (yaml) on {len(files)} template files: " + ", ".join(f"{k} {c[k]}" for k in ("unchanged", "changed", "parse error")))


if __name__ == "__main__":
    main()
