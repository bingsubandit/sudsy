"""Findings of an upstream tool on its corpus for the generic coverage mode (scripts/parity_lint.py `coverage`).

A tool entry of parity_tools.json has a `coverage` object (docs/ai/development/parity.md#coverage-mode):
`corpus` (sources: `dir` below the repo root, `glob` patterns, `skip` substrings of the path inside `dir`, `esm` `[module, script]` suffixes by content (ESLint), `invalid_marker` (the file name next to an input that marks it invalid: the proof run leaves those out when Sudsy rejects them), `reference_parser` (a command with `{file}` that exits non-zero on a syntax error: a file Sudsy rejects and it rejects too is invalid, as a marked one), `as` the folder `dir` is copied to (default: its own name), `into` a folder the files are copied into flat: the one a language is found by), `command`
(`{exe}`, `{prefix}`, `{group}` and `{files}`), `group` (a regex over the file's path: files of one group run in one
command; `cwd: group` runs it in the folder `group` names), `chunk` (files per command), `ok_codes` (exit codes with an empty report meaning no findings), `timeout` (seconds) and `memory_gb` (address space) per command, `env`, `stream` (`stdout` or `stderr`: where the report is), `files` (config files
written next to the corpus), `group_files` (config files written into the folder of each group, `{group}` and `{prefix}` replaced: a project's own config), `parse` (how the report turns into findings) and `sudsy` (the Sudsy side's config).
"""
import json
import os
import re
import resource
import shutil
import subprocess
import sys
import tempfile
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def corpus_files(spec, filters):
    """`[(path inside the work folder, source)]`: the sources of the corpus, named from the folder of the corpus; `filters` counts the `skip` ones."""
    out = []
    for source in spec["corpus"]:
        base = ROOT / source["dir"]
        for pattern in source["glob"]:
            for p in sorted(base.rglob(pattern)):
                inside = p.relative_to(base).as_posix()
                if p.is_file() and any(s in inside for s in source.get("skip", [])):
                    filters["skip"] += 1
                elif p.is_file():
                    name = f"{source['as']}/{inside}" if "as" in source else p.relative_to(base.parent).as_posix()
                    if "suffix" in source and not name.endswith(source["suffix"]):  # the tool and Sudsy read the file by its name (`*.component.html` is Angular)
                        name = name.removesuffix(Path(name).suffix) + source["suffix"]
                    if "esm" in source:  # `[module suffix, script suffix]`: a `.js` with `import`/`export` is read as a module by both tools, the others as scripts
                        module = re.search(r"^(?:import|export)\b", p.read_text(encoding="utf-8", errors="replace"), re.M)
                        name = str(Path(name).with_suffix(source["esm"][0 if module else 1]))
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
    """`[(file, rule, message, line)]` of a report (`line` is None when the entry gives none). `regex`: one finding per matching line (groups `file`, `rule`,
    `message`, and `line`). `json` and `jsonl`: `levels` name the lists to walk (`""` the report itself; `files*` an object
    whose members are the elements), `file`, `rule` and `message` are `<level>.<keys>` and `rule_fn` names a
    function of `rules` that takes the finding's record, `line` is `<level>.<keys>` too. Both: `rule_prefix` is put before the rule and
    `rule_sub` is a `[pattern, replacement]` over it, `rule_message` maps a message pattern to the id the finding is reported under, `json_line` reads the report from the last line that starts an object; a finding with no rule (a parse error) is left out."""
    found = []
    if spec["format"] == "regex":
        found = [(m["file"], m["rule"], m.groupdict().get("message") or "", m.groupdict().get("line")) for m in (re.match(spec["pattern"], line) for line in output.splitlines()) if m]
    else:
        if spec.get("json_line"):  # a report after text the tool prints around it: the last line that starts an object
            output = next((line for line in reversed(output.splitlines()) if line.startswith("{")), "")
            if not output:
                raise ValueError("no JSON object in the report")
        data = [json.loads(line) for line in output.splitlines() if line.startswith("{")] if spec["format"] == "jsonl" else json.loads(output) if output.strip() else []
        for chain in walk(data, spec["levels"], []):
            rule = rules[spec["rule_fn"]](chain[-1]) if "rule_fn" in spec else str(ref(chain, spec["rule"]))
            found.append((str(ref(chain, spec["file"])), rule, str(ref(chain, spec["message"]) or ""), ref(chain, spec["line"]) if "line" in spec else None))
    out = []
    for file, rule, message, line in found:
        for pattern, sub in spec.get("rule_message", {}).items():  # one upstream rule reports as the sub-rule whose message it is
            if re.search(pattern, message):
                rule = sub
        if "rule_sub" in spec:
            rule = re.sub(spec["rule_sub"][0], spec["rule_sub"][1], rule)
        if rule != "None":
            out.append((file, spec.get("rule_prefix", "") + rule, message, int(line) if line is not None else None))
    return out


def proof(tool, spec, prefix, work, exe, rules, sudsy, preset, layout, left_out, before, pinned=lambda f: False, named=None):
    """The proof run of docs/ai/design/parity-target.md (every coverage run of a tool with `formatter` entries): the corpus of `work` is copied, formatted with
    `sudsy format` under the tool's preset and checked by the tool again; returns the findings of `layout` ids (the `formatter`
    catalog entries) that remain. A finding left counts as missing in the target, unless `pinned(finding)` (a reviewed `tool-wrong` one);
    `before` is the same count before formatting."""
    after = prefix / "proof"
    shutil.rmtree(after, ignore_errors=True)
    shutil.copytree(work, after)
    config = {"extends": [f"preset:{preset}"]}
    if spec.get("overrides"):  # per-folder settings the tool gets from its own flags (sqlfluff: `--dialect`), as in the lint run
        config["overrides"] = spec["overrides"]
    (after / "sudsy.yaml").write_text(json.dumps(config), encoding="utf-8")
    run = subprocess.run([sudsy, "format", "."], cwd=after, capture_output=True, text=True, encoding="utf-8", check=False)
    (after / "sudsy.yaml").unlink()
    found, _ = findings(tool, spec, prefix, after, exe, rules, copy=False)
    # A file `sudsy format` rejects stays as it was. The reference for "invalid" is the corpus's own marking (`invalid_marker`: a file of
    # that name next to the input, as the yaml-test-suite's `error`): a marked file is invalid input and leaves the count, any other
    # rejected file is a Sudsy bug and its findings stay.
    sources = dict(corpus_files(spec, Counter()))
    rejected = {n for n in (norm(m, after) for m in re.findall(r"^error: (.+?): \d+:\d+:", run.stderr, re.M)) if n in sources}
    marker = spec.get("invalid_marker")
    invalid = {n for n in rejected if marker and (sources[n].parent / marker).exists()}
    reference = spec.get("reference_parser")  # a command (`{file}`) that exits non-zero on a syntax error: the language's own parser
    invalid |= {n for n in rejected - invalid if reference and subprocess.run([a.format(python=sys.executable, file=sources[n]) for a in reference], capture_output=True, check=False).returncode}
    left = [f for f in found if f[1] in layout and f[0] not in left_out and f[0] not in invalid and not pinned(f)]
    per = Counter(f[1] for f in left)
    print(f"proof: `sudsy format` exit {run.returncode}; formatter findings {len(before)} before, {len(left)} after in {len({f[0] for f in left})} files: "
          + (", ".join(f"{r} {n}" for r, n in per.most_common(12)) or "none"))
    print(f"proof: `sudsy format` rejects {len(rejected)} files: {len(invalid)} that the corpus marks or the reference parser rejects (invalid-input, left out), "
          f"{len(rejected) - len(invalid)} that it does not (named gaps or Sudsy bugs, counted)"
          + "".join(f"\n  {'gap ' + ','.join(named[n]) if n in (named or {}) else 'bug'}: {n}" for n in sorted(rejected - invalid)))
    return left


def norm(path, work):
    """A path as the work folder names it: relative, with `/`."""
    p = os.path.normpath(path)
    return (os.path.relpath(p, work) if os.path.isabs(p) else p).replace("\\", "/")


def findings(tool, spec, prefix, work, exe, rules, copy=True):
    """Copies the corpus into `work` (unless `copy` is off: the files are there already), writes the config files and runs the tool;
    returns `(findings, files)` where `findings` is `[(path, rule, message, line)]` and `files` the number of corpus files."""
    filters = Counter()
    files = corpus_files(spec, filters)
    for inside, source in files if copy else []:
        (work / inside).parent.mkdir(parents=True, exist_ok=True)
        shutil.copy(source, work / inside)
    for name, text in spec.get("files", {}).items() if copy else []:
        (work / name).write_text(text.replace("{prefix}", str(prefix)), encoding="utf-8")
    groups = {}
    for inside, _ in files:
        m = re.search(spec["group"], inside) if "group" in spec else None
        groups.setdefault(m.group(1) if m else "", []).append(inside)
    for group in groups if copy else []:
        for name, text in spec.get("group_files", {}).items():
            (work / group / name).write_text(text.replace("{prefix}", str(prefix)).replace("{group}", group), encoding="utf-8")
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
                found += [(norm(f if os.path.isabs(f) or not folder else f"{folder}/{f}", work), r, m, n) for f, r, m, n in parse(report, spec["parse"], rules)]
            except (json.JSONDecodeError, KeyError, ValueError) as error:
                raise SystemExit(f"{tool}: no usable report ({error}):\n{report[:500]}\n{stderr[:500]}")
    print(f"input filters: skip {filters['skip']}, the tool did not finish (time or memory limit, or a crash) {len(skipped)}")
    if skipped:
        print(f"the tool did not finish (time or memory limit, or a crash) on {len(skipped)} file(s), which are left out: {', '.join(skipped)}")
    return found, len(files) - len(skipped)
