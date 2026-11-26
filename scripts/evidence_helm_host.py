"""YAML host measurement of the Helm parser candidates (docs/ai/design/helm.md): can the YAML of a template be parsed once its
actions are masked, by the existing Sudsy YAML parser (the candidate that reads the host as embedded regions)?

Usage: python3 -I scripts/evidence_helm_host.py <sudsy-binary> <work-dir>

Corpus: the evidence's files (`scripts/evidence.py helm list`, the cap included). Two runs with `sudsy format` on a copy of each
file (exit status 0: the text parsed):
- raw: the template as written (Go template syntax is not YAML, so the rate is low by construction);
- masked: each action that stands alone on its line becomes a blank line, every other action becomes `x` characters of its
  length (newlines kept), so the line count and the columns of the YAML text do not move.
"""
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import evidence  # noqa: E402
import evidence_helm  # noqa: E402


def mask(text):
    """The YAML host of a template: own-line actions blanked, other actions as `x` (newlines kept)."""
    chars = list(text)
    for a, b, _ in evidence_helm.actions(text):
        ls = text.rfind("\n", 0, a) + 1
        le = text.find("\n", b)
        le = len(text) if le < 0 else le
        own = text[ls:a].strip() == "" and text[b:le].strip() == ""
        fill = " " if own else "x"
        for k in range(a, b):
            if chars[k] != "\n":
                chars[k] = fill
    return "".join(chars)


def parses(sudsy, work, name, text):
    d = work / name
    d.mkdir(parents=True, exist_ok=True)
    f = d / "t.yaml"
    f.write_text(text, encoding="utf-8", newline="\n")
    r = subprocess.run([sudsy, "format", "t.yaml"], cwd=d, capture_output=True, text=True, encoding="utf-8", check=False)
    return r.returncode == 0


def main():
    sudsy, work = sys.argv[1], Path(sys.argv[2])
    corpus = evidence.files(evidence_helm)
    texts = [evidence.read(f, True) for _, _, f in corpus]
    pairs = [(i, t) for i, t in enumerate(texts) if t is not None]
    with ThreadPoolExecutor(8) as ex:
        raw = list(ex.map(lambda p: parses(sudsy, work, f"raw{p[0]}", p[1]), pairs))
        masked = list(ex.map(lambda p: parses(sudsy, work, f"m{p[0]}", mask(p[1])), pairs))
    n = len(pairs)
    print(f"files read: {n} of {len(corpus)}")
    print(f"raw (as written): parses {sum(raw)} ({100 * sum(raw) / max(n, 1):.1f}%)")
    print(f"masked (actions removed from the YAML text): parses {sum(masked)} ({100 * sum(masked) / max(n, 1):.1f}%)")


if __name__ == "__main__":
    main()
