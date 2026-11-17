"""Findings of an upstream tool on its corpus for the generic coverage mode (scripts/parity_lint.py `coverage`).

A tool entry of parity_tools.json has a `coverage` object (docs/development/parity.md#coverage-mode):
`corpus` (sources: `dir` below the repo root, `glob` patterns, `skip` substrings of the path inside `dir`, `as` the folder `dir` is copied to (default: its own name), `into` a folder the files are copied into flat: the one a language is found by), `command`
(`{exe}`, `{prefix}`, `{group}` and `{files}`), `group` (a regex over the file's path: files of one group run in one
command; `cwd: group` runs it in the folder `group` names), `chunk` (files per command), `ok_codes` (exit codes with an empty report meaning no findings), `timeout` (seconds) and `memory_gb` (address space) per command, `env`, `stream` (`stdout` or `stderr`: where the report is), `files` (config files
written next to the corpus), `parse` (how the report turns into findings) and `sudsy` (the Sudsy side's config).
"""
import json
import os
import re
import resource
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def corpus_files(spec):
    """`[(path inside the work folder, source)]`: the sources of the corpus, named from the folder of the corpus."""
    out = []
    for source in spec["corpus"]:
        base = ROOT / source["dir"]
        for pattern in source["glob"]:
            for p in sorted(base.rglob(pattern)):
                inside = p.relative_to(base).as_posix()
                if p.is_file() and not any(s in inside for s in source.get("skip", [])):
                    name = f"{source['as']}/{inside}" if "as" in source else p.relative_to(base.parent).as_posix()
                    out.append((f"{source['into']}/{name.replace('/', '__')}" if "into" in source else name, p))
    return list(dict(out).items())


def ref(chain, path):
    """`0.rule.name`: the element of the chain at the level, then the dotted keys."""
    level, _, keys = path.partition(".")
    node = chain[int(level)]
    for key in keys.split(".") if keys else []:
        node = node.get(key) if isinstance(node, dict) else None
    return node


def walk(node, levels, chain):
    if not levels:
        yield chain
        return
    key = levels[0]
    if key.endswith("*"):  # an object whose keys are the elements (a file name), put in the element as `_key`
        items = [{**v, "_key": k} for k, v in (node.get(key[:-1]) or {}).items()] if isinstance(node.get(key[:-1]), dict) else []
    else:
        items = node if key == "" else (node.get(key) or [])
    for item in items:
        yield from walk(item, levels[1:], chain + [item])


def parse(output, spec, rules):
    """`[(file, rule, message)]` of a report. `regex`: one finding per matching line (groups `file`, `rule`,
    `message`). `json` and `jsonl`: `levels` name the lists to walk (`""` the report itself; `files*` an object
    whose members are the elements), `file`, `rule` and `message` are `<level>.<keys>` and `rule_fn` names a
    function of `rules` that takes the finding's record. Both: `rule_prefix` is put before the rule and
    `rule_sub` is a `[pattern, replacement]` over it, `json_line` reads the report from the last line that starts an object; a finding with no rule (a parse error) is left out."""
    found = []
    if spec["format"] == "regex":
        found = [(m["file"], m["rule"], m.groupdict().get("message") or "") for m in (re.match(spec["pattern"], line) for line in output.splitlines()) if m]
    else:
        if spec.get("json_line"):  # a report after text the tool prints around it: the last line that starts an object
            output = next((line for line in reversed(output.splitlines()) if line.startswith("{")), "")
        data = [json.loads(line) for line in output.splitlines() if line.startswith("{")] if spec["format"] == "jsonl" else json.loads(output) if output.strip() else []
        for chain in walk(data, spec["levels"], []):
            rule = rules[spec["rule_fn"]](chain[-1]) if "rule_fn" in spec else str(ref(chain, spec["rule"]))
            found.append((str(ref(chain, spec["file"])), rule, str(ref(chain, spec["message"]) or "")))
    out = []
    for file, rule, message in found:
        if "rule_sub" in spec:
            rule = re.sub(spec["rule_sub"][0], spec["rule_sub"][1], rule)
        if rule != "None":
            out.append((file, spec.get("rule_prefix", "") + rule, message))
    return out


def norm(path, work):
    """A path as the work folder names it: relative, with `/`."""
    p = os.path.normpath(path)
    return (os.path.relpath(p, work) if os.path.isabs(p) else p).replace("\\", "/")


def findings(tool, spec, prefix, work, exe, rules):
    """Copies the corpus into `work`, writes the config files and runs the tool; returns `(findings, files)`
    where `findings` is `[(path, rule, message)]` and `files` the number of corpus files."""
    files = corpus_files(spec)
    for inside, source in files:
        (work / inside).parent.mkdir(parents=True, exist_ok=True)
        shutil.copy(source, work / inside)
    for name, text in spec.get("files", {}).items():
        (work / name).write_text(text.replace("{prefix}", str(prefix)), encoding="utf-8")
    groups = {}
    for inside, _ in files:
        m = re.search(spec["group"], inside) if "group" in spec else None
        groups.setdefault(m.group(1) if m else "", []).append(inside)
    limit = (lambda: resource.setrlimit(resource.RLIMIT_AS, (spec["memory_gb"] << 30,) * 2)) if "memory_gb" in spec else None
    found, skipped = [], []
    size = spec.get("chunk", 200)
    for group, names in groups.items():
        # a command without `{files}` runs once over the folder
        for i in range(0, len(names), size) if "{files}" in spec["command"] else [0]:
            folder = group if spec.get("cwd") == "group" else ""  # a group that is a project of its own (buf: one module)
            command = []
            for arg in spec["command"]:
                if arg == "{files}":
                    command += [n[len(folder) + 1:] if folder else n for n in names[i:i + size]]
                else:
                    command.append(arg.format(exe=exe, prefix=prefix, group=group, python=sys.executable, work=work, root=ROOT))
            env = {**os.environ, **{k: v.format(prefix=prefix) for k, v in spec.get("env", {}).items()}}
            with tempfile.TemporaryFile("w+", encoding="utf-8", errors="replace") as out, tempfile.TemporaryFile("w+", encoding="utf-8", errors="replace") as err:
                try:  # files, not pipes: a Node tool that exits after a write to a pipe cuts its report at 64 KiB
                    run = subprocess.run(command, cwd=work / folder, env=env, stdout=out, stderr=err, check=False, timeout=spec.get("timeout", 900), preexec_fn=limit)
                except subprocess.TimeoutExpired:  # a tool that does not finish (shellcheck on nvm.sh) leaves its files out, and they are listed
                    skipped += names[i:i + size] if "{files}" in spec["command"] else names
                    continue
                out.seek(0)
                err.seek(0)
                stdout, stderr = out.read(), err.read()
            report = stderr if spec.get("stream") == "stderr" else stdout
            if not report.strip() and run.returncode:
                print(f"{tool}: no report from {group or 'the corpus'} (exit {run.returncode}): {stderr.strip()[:300]}", file=sys.stderr)
            if not report.strip() and run.returncode not in spec.get("ok_codes", [0, 1]):  # killed (out of memory) or failed: no report
                skipped += names[i:i + size] if "{files}" in spec["command"] else names
                continue
            try:
                found += [(norm(f if os.path.isabs(f) or not folder else f"{folder}/{f}", work), r, m) for f, r, m in parse(report, spec["parse"], rules)]
            except (json.JSONDecodeError, KeyError, ValueError) as error:
                raise SystemExit(f"{tool}: no usable report ({error}):\n{report[:500]}\n{stderr[:500]}")
    if skipped:
        print(f"the tool did not finish (time or memory limit, or a crash) on {len(skipped)} file(s), which are left out: {', '.join(skipped)}")
    return found, len(files) - len(skipped)
