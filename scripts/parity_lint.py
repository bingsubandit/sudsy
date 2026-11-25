"""Compares an upstream tool with `sudsy lint` on tests/parity/<tool>/ (run by scripts/parity.sh).

Usage: parity_lint.py <tool> <sudsy-binary> <target-dir> [--coverage]

The tool's version, install and corpora come from its entry in parity_tools.json (parity_format.prepare).
"""
import collections
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

import yaml

import parity_coverage
import parity_format
import parity_landing

ROOT = Path(__file__).resolve().parent.parent
PARITY = ROOT / os.environ.get("CARGO_TARGET_DIR", "target") / "parity"


def catalog_entries(tool, sub=False):
    catalog = yaml.safe_load((ROOT / "docs/ai/parity" / f"{parity_format.tools()['lint'][tool]['catalog']}.yaml").read_text(encoding="utf-8"))
    out = []
    for id_, value in catalog["entries"].items():  # `<status> [| <gap>]`, see docs/ai/parity/README.md
        status, _, gap = value.partition(" | ")
        kind, _, arg = status.partition(" ")
        coverage = kind if kind in ("sdk", "wontfix", "deferred") else "missing" if kind == "planned" else "partial" if gap else "covered"
        out.append({"id": id_, "sudsy": f"{kind}: {arg}" if arg else kind, "coverage": coverage})
    for group in catalog.get("deferred", []):
        out += [{"id": i, "sudsy": "deferred", "coverage": "deferred"} for i in group["ids"]]
    return [e for e in out if sub or "." not in e["id"]]


def rule_map(tool):
    """Upstream rule name (`anchors.forbid-unused-anchors` for an option that is a rule of its own) -> Sudsy
    rule id, from the catalog."""
    return {
        e["id"]: e["sudsy"][len("rule: "):].split(", ")[0]  # an entry naming several ids is compared on the first
        for e in catalog_entries(tool, sub=True)
        if e["sudsy"].startswith("rule: ")
    }


def rule_aliases(tool):
    """Sudsy rule id -> the first id of its catalog entry, for the entries that name several ids (an upstream
    rule that Sudsy reports under more than one id: ESLint's `no-unused-vars`)."""
    out = {}
    for e in catalog_entries(tool, sub=True):
        if e["sudsy"].startswith("rule: "):
            ids = e["sudsy"][len("rule: "):].split(", ")
            out.update({i: ids[0] for i in ids[1:]})
    return out


def formatter_ids(tool):
    """`(covered, partial)`: upstream rule names the formatter handles, from the catalog: status
    `formatter`, or `option: csharp.format.<name>` for an option the stylecop preset sets (the
    fixtures format with it). `partial` entries state a gap."""
    preset = yaml.safe_load((ROOT / "crates/lang-csharp/presets/stylecop.yaml").read_text(encoding="utf-8"))
    options = set(preset["csharp"]["format"])
    def ids(coverage):
        return {
            e["id"]
            for e in catalog_entries(tool)
            if e["coverage"] == coverage
            and (e["sudsy"] == "formatter" or e["sudsy"].removeprefix("option: csharp.format.") in options)
        }
    return ids("covered"), ids("partial")


def yamllint(fixture, cwd, prefix):
    """`{(upstream rule, line, column)}` of the fixture."""
    out = subprocess.run(
        [sys.executable, "-m", "yamllint", "-f", "parsable", "-c", ".yamllint", fixture.name],
        cwd=cwd, capture_output=True, text=True, check=False, env={**os.environ, "PYTHONPATH": str(prefix)},
    ).stdout
    found = set()
    for line in out.splitlines():
        m = re.match(r"^.+?:(\d+):(\d+): \[\w+\] (.*?)(?: \(([\w-]+)\))?$", line)
        if m:
            rule, column = m.group(4) or "syntax-error", int(m.group(2))
            if rule == "anchors":  # one yamllint rule, three Sudsy rules: "found unused anchor ..."
                rule += {"undeclared": ".forbid-undeclared-aliases", "duplicated": ".forbid-duplicated-anchors", "unused": ".forbid-unused-anchors"}[m.group(3).split()[1]]
            if rule in ("braces", "brackets") and "forbidden" in m.group(3):  # the `forbid` option
                rule += ".forbid"
                column -= 1  # after the delimiter
            # yamllint reports these two after the token: Sudsy reports the token (the `:` or `-`,
            # the octal number).
            if rule == "empty-values":
                column -= 1
            elif rule == "octal-values":
                column -= len(re.search(r'"(.*)"$', m.group(3)).group(1))
            found.add((rule, int(m.group(1)), column))
    return found


def eslint_jsonc(install, folder):
    """`{fixture name: {(upstream rule, line, column)}}` of the `.json` fixtures. ESLint and the plugin
    are installed in `install`; the fixtures and the flat config are copied below it so that the
    config finds the plugin."""
    work = install / "work"
    shutil.rmtree(work, ignore_errors=True)
    shutil.copytree(folder, work, ignore=shutil.ignore_patterns("sudsy.yaml"))
    run = subprocess.run(
        ["node", str(install / "node_modules/eslint/bin/eslint.js"), "-f", "json", "."],
        cwd=work, capture_output=True, text=True, check=False,
    )
    try:
        results = json.loads(run.stdout)
    except json.JSONDecodeError:
        sys.exit(f"eslint produced no report:\n{run.stdout}\n{run.stderr}")
    return {
        Path(r["filePath"]).name: {(m["ruleId"] or "syntax-error", m["line"], m["column"]) for m in r["messages"]}
        for r in results
    }


def markdownlint(install, folder):
    """`{fixture name: {(upstream rule, line, 0)}}` of the `.md` fixtures (markdownlint reports a column
    for some rules only, so the column is left out of the comparison; Sudsy's is zeroed alike).
    markdownlint-cli2 is installed in `install`; the fixtures and their `.markdownlint.jsonc` are
    copied below it."""
    work = install / "work"
    shutil.rmtree(work, ignore_errors=True)
    shutil.copytree(folder, work, ignore=shutil.ignore_patterns("sudsy.yaml"))
    run = subprocess.run(
        ["node", str(install / "node_modules/markdownlint-cli2/markdownlint-cli2-bin.mjs"), "*.md"],
        cwd=work, capture_output=True, text=True, check=False,
    )
    found = {p.name: set() for p in work.glob("*.md")}
    for line in (run.stdout + run.stderr).splitlines():
        m = re.match(r"^(\S+\.md):(\d+)(?::\d+)? \S+ (MD\d+)", line)
        if m:
            found[m.group(1)].add((m.group(3), int(m.group(2)), 0))
    return found


def corpus_sources(tool, suffixes=()):
    """`[(label, work file name, path)]` of the real-world corpus of ruff, eslint and typescript-eslint (fetched by
    tests/conformance/fetch-js-corpus.sh and fetch-python-corpus.sh); nothing for the other tools."""
    def listing(corpus, files):
        return [(f"corpus/{p.relative_to(corpus).as_posix()}", "-".join(p.relative_to(corpus).parts), p) for p in files]
    if tool in ("eslint", "typescript-eslint"):  # Prettier's own tests include proposal syntax the parsers reject
        corpus = ROOT / "tests/conformance/corpus/js"
        return listing(corpus, [p for suffix in suffixes for p in sorted(corpus.rglob(f"*{suffix}")) if p.is_file() and p.relative_to(corpus).parts[0] != "prettier"])
    if tool in PLUGINS:  # the Vue and Svelte projects of the HTML corpus; Prettier's own format tests include proposal syntax and odd cases
        corpus = ROOT / "tests/conformance/corpus/html"
        return listing(corpus, [p for suffix in suffixes for p in sorted(corpus.rglob(f"*{suffix}")) if p.is_file() and p.relative_to(corpus).parts[0] != "prettier"])
    if tool == "ruff":  # the real-world projects of the format parity (tests/conformance/fetch-python-corpus.sh)
        corpus = ROOT / "tests/conformance/corpus/python"
        roots = ("django/django/", "requests/src/", "flask/src/", "click/src/", "httpx/httpx/", "pytest/src/", "black/src/")
        return listing(corpus, [p for p in sorted(corpus.rglob("*.py")) if p.is_file() and p.relative_to(corpus).as_posix().startswith(roots)])
    return []


PLUGINS = {"eslint-plugin-vue": "vue", "eslint-plugin-svelte": "svelte"}  # tool -> its language, the prefix of its rule names
ENABLE_MAPPED = {"ruff": "python", "eslint": "js", "typescript-eslint": "js"}  # tool -> the config key of its language: the tool runs every mapped rule, so the Sudsy side turns every mapped rule on too


def plugin_config(tool, last):
    """The flat config of a plugin on `.vue` or `.svelte` files: its base config (parser, processor), the TypeScript
    parser for the `lang="ts"` scripts, no inline comments, and `last` (a config object: the rules to run)."""
    name = PLUGINS[tool]
    return (f'import {name} from "{tool}";\nimport tseslint from "typescript-eslint";\n'
            f'export default [...{name}.configs["flat/base"], {{ linterOptions: {{ noInlineConfig: true }}, files: ["**/*.{name}"], '
            f'languageOptions: {{ parserOptions: {{ parser: tseslint.parser }} }} }}, {last}];\n')


def rule_fixtures(tool, sudsy, version, prefix, exe):
    """Runs the tool (hadolint, shellcheck, checkmake, stylelint, html-validate) and `sudsy lint` on the rule fixtures of its language
    (tests/rules/<lang>/<folder>/*<suffix>, annotations removed) and compares `(rule, line)` of the
    catalog's rules: upstream codes the catalog does not map are left out.
    `tests/parity/<tool>/differences.yaml` lists known differences under the key
    `<folder>/<name> <tool|sudsy>:<rule> <line>`; one that no longer differs fails."""
    lang, suffix, command, parse, load = {
        "hadolint": ("dockerfile", ".dockerfile", [exe, "--no-fail", "--no-color", "-f", "json"],
                     lambda d: (d["code"], d["line"]), json.loads),
        "shellcheck": ("shell", ".sh", [exe, "-s", "bash", "-o", "all", "-f", "json"],
                       lambda d: (f"SC{d['code']}", d["line"]), json.loads),
        # one JSON object per line; --path limits the findings to the fixture, the rest of the module is its imports
        "buf": ("protobuf", ".proto", [exe, "lint", "--error-format=json", "--path"],
                lambda d: (d["type"], d["start_line"]), lambda out: [json.loads(l) for l in out.splitlines()]),
        # shellcheck and pyflakes are the shell and python checks of the scripts, which Sudsy's Shell language does
        "actionlint": ("github-actions", ".yml", [exe, "-no-color", "-format", "{{json .}}", "-shellcheck=", "-pyflakes="],
                       lambda d: (actionlint_check(d), d["line"]), lambda out: json.loads(out) or []),
        # flat config of the catalog's rules (written below); espree parses .js, typescript-eslint's parser .ts and .tsx
        # (without type information: only the rules that need none are in the catalog's `rule:` entries)
        **{t: ("js", suffixes, ["node", f"{prefix}/node_modules/eslint/bin/eslint.js", "-c", "eslint.config.mjs", "-f", "json", "--no-warn-ignored"],
               lambda d: (d["ruleId"], d["line"]), lambda out: [m for r in json.loads(out) for m in r["messages"] if m["ruleId"]])
           for t, suffixes in (("eslint", (".js",)), ("typescript-eslint", (".ts", ".tsx")))},
        # stylelint writes its JSON report to stderr; the plugins and the dialect parser come from the config written below
        **{t: (lang, "." + lang, ["node", f"{prefix}/node_modules/stylelint/bin/stylelint.mjs", "--config", "stylelint.config.mjs", "-f", "json"],
               lambda d: (d["rule"], d["line"]), lambda out: [w for r in json.loads(out) for w in r["warnings"]])
           for t, lang in (("stylelint", "css"), ("stylelint-scss", "scss"), ("stylelint-less", "less"))},
        # the plugins' rules are `vue/<name>` and `svelte/<name>`; the config is written below
        **{t: (name, "." + name, ["node", f"{prefix}/node_modules/eslint/bin/eslint.js", "-c", "eslint.config.mjs", "-f", "json", "--no-warn-ignored"],
               lambda d: (d["ruleId"], d["line"]), lambda out: [m for r in json.loads(out) for m in r["messages"] if m["ruleId"]])
           for t, name in PLUGINS.items()},
        # one report per file: a list of messages with the rule id and line; the config is written below
        "html-validate": ("html", ".html", ["node", f"{prefix}/node_modules/html-validate/bin/html-validate.mjs", "-c", "htmlvalidate.json", "-f", "json"],
                          lambda d: (d["ruleId"], d["line"]), lambda out: [m for r in json.loads(out) for m in r["messages"]]),
        # one report for the whole folder: a list of findings with the code, file and row; --select is added below
        "ruff": ("python", ".py", parity_format.tool_command(parity_format.tools()["lint"]["ruff"], prefix)[0],
                 lambda d: (d["code"], d["location"]["row"]), json.loads),
        "checkmake": ("makefile", ".mk", [exe, "--format", '{"rule":"{{.Rule}}","line":{{.LineNumber}}}'],
                      lambda d: (d["rule"], d["line"]),
                      lambda out: [json.loads(l) for l in out.splitlines() if l.startswith("{")]),
    }[tool]
    suffixes = (suffix,) if isinstance(suffix, str) else suffix
    mapping = rule_map(tool)
    aliases = rule_aliases(tool)
    if tool == "ruff":
        command = command + ["--select", ",".join(c for c in mapping if c != "E999")]  # E999 is gone: ruff reports a syntax error without a code
    columns = tool in ("eslint", "typescript-eslint")  # the other tools' columns are not compared
    if tool in PLUGINS:
        mapping = {f"{PLUGINS[tool]}/{r}": v for r, v in mapping.items()}
    if tool == "typescript-eslint":  # the plugin's rules are `@typescript-eslint/<name>`, the catalog's ids the names
        mapping = {f"@typescript-eslint/{r}": v for r, v in mapping.items()}
    folder = ROOT / "tests/parity" / tool
    work = prefix / "work"
    shutil.rmtree(work, ignore_errors=True)
    work.mkdir(parents=True)
    rules = json.dumps({r: "error" for r in mapping})
    if tool == "eslint":
        (work / "eslint.config.mjs").write_text(f"export default [{{ linterOptions: {{ noInlineConfig: true }}, rules: {rules} }}];\n", encoding="utf-8")
    if tool == "typescript-eslint":
        (work / "eslint.config.mjs").write_text(
            'import tseslint from "typescript-eslint";\n'
            f'export default [{{ linterOptions: {{ noInlineConfig: true }}, files: ["**/*.ts", "**/*.tsx"], languageOptions: {{ parser: tseslint.parser }}, plugins: {{ "@typescript-eslint": tseslint.plugin }}, rules: {rules} }}];\n',
            encoding="utf-8")
    if tool in PLUGINS:
        (work / "eslint.config.mjs").write_text(plugin_config(tool, f"{{ rules: {rules} }}"), encoding="utf-8")
    if tool == "html-validate":
        (work / "htmlvalidate.json").write_text(json.dumps({"root": True, "rules": {r: "error" for r in mapping}}), encoding="utf-8")
    if tool.startswith("stylelint"):
        plugin = {"stylelint-scss": 'plugins: ["stylelint-scss"], customSyntax: "postcss-scss", ', "stylelint-less": 'plugins: ["stylelint-less"], customSyntax: "postcss-less", '}.get(tool, "")
        # every mapped rule runs with `true`, or with its options from `rules.json` (which `sudsy.yaml` mirrors)
        options = json.loads((folder / "rules.json").read_text(encoding="utf-8")) if (folder / "rules.json").exists() else {}
        (work / "stylelint.config.mjs").write_text(f"export default {{ {plugin}rules: {json.dumps({r: options.get(r, True) for r in mapping})} }};\n", encoding="utf-8")
    for config in ("sudsy.yaml", "buf.yaml"):
        if (folder / config).exists():
            shutil.copy(folder / config, work / config)
    if tool in ENABLE_MAPPED:  # a rule mapped later needs no line in sudsy.yaml
        data = yaml.safe_load((work / "sudsy.yaml").read_text(encoding="utf-8"))
        rules = data.setdefault(ENABLE_MAPPED[tool], {}).setdefault("lint", {}).setdefault("rules", {})
        for rule in (set(mapping.values()) | set(aliases)) - {"syntax-error"}:  # the parser's diagnostic is no rule
            rules.setdefault(rule, "warn")
        (work / "sudsy.yaml").write_text(yaml.safe_dump(data), encoding="utf-8", newline="\n")
    diffs = parity_format.Differences(folder, lint=True)
    mismatches, total = 0, 0
    sources = [(f"{s.parent.name}/{s.stem}", f"{s.parent.name}-{s.stem}{s.suffix}", s) for suffix in suffixes for s in sorted((ROOT / "tests/rules" / lang).glob(f"*/*{suffix}"))
               if not s.with_suffix(".rules.yaml").exists()]  # a case that sets rule options: both tools run with their defaults here
    if tool == "actionlint":  # the corpus too (tests/conformance/fetch-github-actions-corpus.sh), a file per name
        corpus = ROOT / "tests/conformance/corpus/github-actions"
        sources += [(f"corpus/{p.relative_to(corpus).as_posix()}", "-".join(p.relative_to(corpus).parts), p) for p in sorted(corpus.rglob("*.y*ml")) if p.is_file()]
    if tool in PLUGINS:  # only the cases of the tool's own folder: the rule fixtures are not valid files for the plugin's parser, and the corpus is for coverage (a suppression comment of the plugin hides a finding there)
        sources = [(f"{s.stem}", s.name, s) for s in sorted(folder.glob(f"*{suffixes[0]}"))]
    else:
        sources += corpus_sources(tool, suffixes)
    if not mapping:  # nothing to compare yet
        print(f"{len(sources)} {tool} fixtures, no catalog rule is mapped to a Sudsy rule yet ({tool} {version})")
        return 0
    # A workflow is claimed by its folder.
    where = work / ".github" / "workflows" if tool == "actionlint" else work
    where.mkdir(parents=True, exist_ok=True)
    def read(name, source):
        """The fixture's name and text, annotations removed."""
        text = "".join(
            l for l in source.read_text(encoding="utf-8").splitlines(keepends=True) if not re.match(r"\s*(?:#|//)?\s*\^", l)
        )
        if tool == "eslint":  # both tools read a `.js` as a module by default: a file with `import`/`export` is one, the others are scripts
            name = str(Path(name).with_suffix(".mjs" if re.search(r"^(?:import|export)\b", text, re.M) else ".cjs"))
        return name, text

    batch, unparsed = None, set()
    if tool == "ruff":  # one run on every file
        for label, name, source in sources:
            name, text = read(name, source)
            (where / name).write_text(text, encoding="utf-8", newline="")
        run = subprocess.run(command + ["."], cwd=work, capture_output=True, text=True, encoding="utf-8", check=False, env=parity_format.tool_command(parity_format.tools()["lint"]["ruff"], prefix)[1])
        batch = {name: [] for _, name, _ in sources}
        for d in json.loads(run.stdout):
            batch.setdefault(Path(d["filename"]).name, []).append(d)
    if tool in ("eslint", "typescript-eslint", *PLUGINS):  # one ESLint run on every file: its start-up costs more than the files
        for label, name, source in sources:
            name, text = read(name, source)
            (where / name).write_text(text, encoding="utf-8", newline="")
        run = subprocess.run(command + ["."], cwd=work, capture_output=True, text=True, encoding="utf-8", check=False)
        try:
            results = json.loads(run.stdout)
        except json.JSONDecodeError:
            sys.exit(f"eslint produced no report:\n{run.stdout}\n{run.stderr}")
        batch = {Path(r["filePath"]).name: r["messages"] for r in results}
        unparsed = {n for n, ms in batch.items() if any(m.get("fatal") for m in ms)}
        print(f"{len(unparsed)} of {len(batch)} files the parser rejects are left out: {' '.join(sorted(unparsed)[:5])}")
    for label, name, source in sources:
        name, text = read(name, source)
        fixture = where / name
        action = tool == "actionlint" and "/.github/actions/" in f"/{label}"  # a composite action is no workflow: actionlint finds none in its project, and Sudsy lints workflows only
        if action:
            fixture = work / ".github" / "actions" / "x" / "action.yaml"
            fixture.parent.mkdir(parents=True, exist_ok=True)
        if batch is None:
            fixture.write_text(text, encoding="utf-8", newline="")
            run = subprocess.run(command + ([] if action else [fixture.relative_to(work).as_posix()]), cwd=work, capture_output=True, text=True, check=False)
            out = run.stdout or run.stderr  # stylelint reports on stderr
            if action and not out.strip().startswith("["):
                out = "[]"
        elif name in unparsed:
            continue
        # checkmake runs every rule on every file: a fixture is compared on the rule of its folder, and
        # checkmake reports a variable (and `.PHONY`, which it reads as one) a line late.
        late = {"timestampexpanded", "minphony"} if tool == "checkmake" else set()
        # ESLint reports a column; the other tools' columns are not compared
        found = [(parse(d), d) for d in (load(out) if batch is None else [m for m in batch.get(name, []) if m.get("ruleId", m.get("code"))])]
        upstream = {(mapping[c], l - (c in late), d["column"] if columns else 0) for (c, l), d in found if c in mapping}
        ours = {(aliases.get(r, r), l, c if columns else 0) for r, l, c in sudsy_lint(sudsy, fixture, work, set(mapping.values()) | set(aliases))}
        if tool == "checkmake":
            upstream, ours = ({row for row in side if row[0] == source.parent.name} for side in (upstream, ours))
        if batch is None:
            fixture.unlink()  # buf compiles the whole folder: fixtures that share a package must not meet
        total += len(upstream)
        problems = []
        diffs.tally(True, count=len(upstream & ours))
        for side, rows in ((tool, upstream - ours), ("sudsy", ours - upstream)):
            for rule, line, column in sorted(rows):
                key = f"{label} {side}:{rule} {line}"
                listed = diffs.known(key)
                diffs.tally(False, diffs.status(key) if listed else None)
                if not listed:
                    problems.append(f"  only {side}: {rule} {line}" + (f":{column}" if column else ""))
        print(f"{label}: {len(upstream)} {tool}, {len(ours)} sudsy: {'MISMATCH' if problems else 'ok'}")
        print("\n".join(problems))
        mismatches += len(problems)
    mismatches += diffs.finish(scope={label for label, _, _ in sources})
    diffs.summary()
    print(f"{total} {tool} findings, {mismatches} mismatch(es) ({tool} {version}, {sum(k in diffs.files for k in diffs.differing)} known)")
    return 1 if mismatches else 0


def actionlint_check(d):
    """The catalog id of an actionlint error: its kind, split by message where one kind holds several checks."""
    kind, message = d["kind"], d["message"]
    if kind == "expression":
        if "potentially untrusted" in message:
            return "expression/untrusted"
        if re.match(r"got unexpected|unexpected end|undefined (variable|function)|expected|.*while (lexing|parsing)", message):
            return "expression/syntax"
        if re.match(r"context .* is not allowed here|calling function .* is not allowed here|.* is only available in", message):
            return "expression/context"
        return "expression/types"
    if kind == "events":
        if re.search(r"CRON|timezone|too frequently", message):
            return "events/schedule"
        if "filters cannot be used" in message:
            return "events/filters"
        if re.search(r"input|default value", message):
            return "events/inputs"
        return "events"
    if kind == "action" and "runner of" in message:
        return "action/outdated-runner"
    if kind == "id":
        return "id/duplicate" if "duplicate" in message else "id/job" if "job ID" in message else "id/step"
    if kind == "job-needs":
        return "job-needs/cycle" if "cyclic" in message else "job-needs/unknown" if "does not exist" in message else "job-needs/duplicate"
    if kind == "runner-label":
        return "runner-label/unknown" if "unknown" in message else "runner-label/conflict"
    if kind == "syntax-check" and "is duplicated" in message:
        return "syntax-check/duplicate-job" if 'in "jobs" section' in message else "syntax-check/duplicate-key"
    if kind == "syntax-check" and re.search(r"for input type of|input type of workflow_dispatch", message):
        return "events/inputs"
    if kind == "syntax-check" and "unknown anchor" in message:
        return "syntax-check/undefined-alias"
    if kind == "syntax-check" and "is defined but not used" in message:
        return "syntax-check/unused-anchor"
    if kind == "syntax-check" and "merge key" in message:
        return "syntax-check/merge-key"
    if kind == "syntax-check" and "recursive alias" in message:
        return "syntax-check/recursive-alias"
    if kind == "syntax-check" and re.search(r"anchor|alias|could not parse", message):
        return "syntax-check/yaml"
    if kind == "syntax-check":
        for pattern, name in (
            (r"should not be empty|is empty", "empty"),
            (r"unexpected key|unexpected alias", "unexpected-key"),
            (r"is missing|expected \"[^\"]*\" key|both .* must be specified|step must run", "missing-key"),
            (r"expecting a single", "expression-value"),
            (r"must be greater than zero", "number"),
            (r"node|must be (string|sequence|mapping)", "kind"),
        ):
            if re.search(pattern, message):
                return "syntax-check/" + name
    return kind


def psscriptanalyzer(sudsy, version, prefix):
    """Invoke-ScriptAnalyzer against `sudsy lint` (preset powershell/psscriptanalyzer) on the PowerShell rule
    fixtures (tests/rules/powershell/*/*.ps1, annotations removed) and the PowerShell corpus, as
    `(file, rule, line)` of the rules the catalog maps. Also checks that the catalog lists exactly the rules
    of the installed module. `tests/parity/psscriptanalyzer/differences.yaml` keys:
    `<file> <psscriptanalyzer|sudsy>:<rule> <line>` (a fixture is `<folder>-<name>.ps1`, a corpus file
    `corpus/<path>`); one that no longer differs fails."""
    script = ROOT / "scripts/parity_psscriptanalyzer.ps1"
    pwsh = lambda *args: subprocess.run(["pwsh", "-NoProfile", "-File", str(script), str(prefix), *args],
                                        capture_output=True, text=True, encoding="utf-8", check=True).stdout
    # The catalog lists the module's rules.
    module = dict(l.split() for l in pwsh("rules").splitlines())
    catalog = {e["id"] for e in catalog_entries("psscriptanalyzer")}
    mismatches = 0
    for rule in sorted(set(module) ^ catalog):
        print(f"  catalog and module differ: {rule} ({'catalog only' if rule in catalog else 'module only'})")
        mismatches += 1
    preset = yaml.safe_load((ROOT / "crates/lang-powershell/presets/psscriptanalyzer.yaml").read_text(encoding="utf-8"))
    mapping = {k: v for k, v in rule_map("psscriptanalyzer").items() if v in preset["powershell"]["lint"]["rules"]}
    folder = ROOT / "tests/parity/psscriptanalyzer"
    work = prefix / "work"
    shutil.rmtree(work, ignore_errors=True)
    work.mkdir(parents=True)
    shutil.copy(folder / "sudsy.yaml", work / "sudsy.yaml")
    corpus = ROOT / "tests/conformance/corpus/powershell"
    for source in sorted((ROOT / "tests/rules/powershell").glob("*/*.ps1")):
        text = "".join(
            l for l in source.read_text(encoding="utf-8").splitlines(keepends=True) if not re.match(r"\s*#?\s*\^", l)
        )
        (work / f"{source.parent.name}-{source.stem}.ps1").write_text(text, encoding="utf-8", newline="")
    shutil.copytree(corpus, work / "corpus")
    diffs = parity_format.Differences(folder, lint=True)
    key_of = lambda p: Path(p).resolve().relative_to(work.resolve()).as_posix()
    upstream = {(key_of(d["file"]), mapping[d["rule"]], d["line"])
                for d in json.loads(pwsh("run", ",".join(mapping), str(work)) or "[]") if d["rule"] in mapping}
    out = subprocess.run([sudsy, "lint", "--format", "json", "."], cwd=work, capture_output=True, text=True,
                         encoding="utf-8", check=False).stdout
    ours = {(d["path"].replace("\\", "/"), d["rule"], d["range"]["start"]["line"])
            for d in json.loads(out)["diagnostics"] if d["rule"] in set(mapping.values())}
    for rule, count in sorted((r, sum(1 for u in upstream if u[1] == r)) for r in set(mapping.values())):
        print(f"{rule}: {count} psscriptanalyzer, {sum(1 for o in ours if o[1] == rule)} sudsy")
    diffs.tally(True, count=len(upstream & ours))
    for side, rows in (("psscriptanalyzer", upstream - ours), ("sudsy", ours - upstream)):
        for path, rule, line in sorted(rows):
            key = f"{path} {side}:{rule} {line}"
            listed = diffs.known(key)
            diffs.tally(False, diffs.status(key) if listed else None)
            if not listed:
                src = (work / path).read_text(encoding="utf-8-sig", errors="replace").splitlines()[line - 1].strip()[:90]
                print(f"  only {side}: {key}  | {src}")
                mismatches += 1
    mismatches += diffs.finish()
    diffs.summary()
    print(f"{len(upstream)} psscriptanalyzer findings, {mismatches} mismatch(es) (psscriptanalyzer {version}, {sum(k in diffs.files for k in diffs.differing)} known)")
    return 1 if mismatches else 0


def sudsy_lint(sudsy, fixture, cwd, only=None):
    """`{(rule, line, column)}`; with `only`, the diagnostics of those rules."""
    out = subprocess.run(
        [sudsy, "lint", "--format", "json", fixture.relative_to(cwd).as_posix()],
        cwd=cwd, capture_output=True, text=True, check=False,
    )
    report = json.loads(out.stdout)
    return {
        (d["rule"], d["range"]["start"]["line"], 0 if fixture.suffix == ".md" else d["range"]["start"]["column"])
        for d in report["diagnostics"]
        if only is None or d["rule"] in only
    }


def compare(label, upstream, ours, tool, diffs):
    """Prints the differences between two `{(rule, line, column)}` sets; returns their count (`diffs` tallies the target)."""
    only_upstream, only_ours = sorted(upstream - ours), sorted(ours - upstream)
    diffs.tally(True, count=len(upstream & ours))
    diffs.tally(False, count=len(only_upstream) + len(only_ours))
    status = "ok" if not (only_upstream or only_ours) else "MISMATCH"
    print(f"{label}: {len(upstream)} {tool}, {len(ours)} sudsy: {status}")
    for rule, line, col in only_upstream:
        print(f"  only {tool}: {rule} {line}:{col}")
    for rule, line, col in only_ours:
        print(f"  only sudsy:  {rule} {line}:{col}")
    return len(only_upstream) + len(only_ours)


def stylecop_build(directory, ids, version):
    """`{(file name, rule, line, column)}` of the StyleCop diagnostics `dotnet build` reports for the
    fixture project in `directory`, restricted to `ids` (each enabled as a warning; the other
    StyleCop rules are noise for the comparison)."""
    (directory / ".editorconfig").write_text(
        "root = true\n[*.cs]\n" + "".join(f"dotnet_diagnostic.{i}.severity = warning\n" for i in sorted(ids)),
        encoding="utf-8",
    )
    log = directory / "build.sarif"
    run = subprocess.run(
        ["dotnet", "build", "--no-incremental", "-nologo", "-v:q", f"-p:StyleCopVersion={version}",
         f"-p:ErrorLog={log}"],
        cwd=directory, capture_output=True, text=True, check=False,
    )
    if not log.exists():
        sys.exit(f"dotnet build produced no error log:\n{run.stdout}\n{run.stderr}")
    found = set()
    for result in json.loads(log.read_text(encoding="utf-8"))["runs"][0]["results"]:
        if result["ruleId"] not in ids:
            continue
        location = result["locations"][0]
        location = location.get("physicalLocation") or location.get("resultFile")
        uri = (location.get("artifactLocation") or location)["uri"]
        region = location["region"]
        name = Path(uri.replace("%20", " ")).name
        found.add((name, result["ruleId"], region["startLine"], region["startColumn"]))
    return found


def stylecop(sudsy, version):
    """Two builds of the fixtures. Plain: every `rule:` id StyleCop reports is reported by `sudsy
    lint` at the same place, and nothing else. After `sudsy fix` (which formats): no `formatter` id
    remains, and what StyleCop still reports is what `sudsy lint` still reports."""
    mapping = rule_map("stylecop")
    formatter, partial = formatter_ids("stylecop")
    mismatches = 0
    diffs = parity_format.Differences(ROOT / "tests/parity/stylecop", lint=True)
    for name in ("plain", "fixed"):
        work = PARITY / f"stylecop-{name}"
        shutil.rmtree(work, ignore_errors=True)
        shutil.copytree(ROOT / "tests/parity/stylecop", work)
        if name == "fixed":
            subprocess.run([sudsy, "fix", "--unsafe", "."], cwd=work, capture_output=True, text=True, check=False)
        built = stylecop_build(work, set(mapping) | formatter | partial, version)
        if name == "plain":
            idle = sorted(formatter - {r for _, r, _, _ in built})
            print(f"formatter ids no fixture violates: {' '.join(idle)}")
        for fixture in sorted(work.glob("*.cs")):
            upstream = {(mapping[r], l, c) for f, r, l, c in built if f == fixture.name and r in mapping}
            mismatches += compare(f"{fixture.name} ({name})", upstream, sudsy_lint(sudsy, fixture, work, set(mapping.values())), "stylecop", diffs)
        if name == "fixed":
            for f, r, l, c in sorted(built):
                # A `partial` entry's gap is only expected outside the `layout_*` fixtures.
                if r in formatter or (r in partial and f.startswith("layout_")):
                    print(f"  formatter id left after fix: {f}: {r} {l}:{c}")
                    mismatches += 1
    diffs.summary()
    print(f"stylecop {version}: {mismatches} mismatch(es)")
    return 1 if mismatches else 0


COVERAGE = {  # tool -> (language, file suffixes, flat config or command line for its default/recommended set)
    "ruff": ("python", (".py",)),
    "eslint": ("js", (".js",)),
    "typescript-eslint": ("js", (".ts", ".tsx")),
    "eslint-plugin-vue": ("vue", (".vue",)),
    "eslint-plugin-svelte": ("svelte", (".svelte",)),
}


def message_name(message):
    """The first name a message quotes (`os`, 'x'), or None; a directive without its shorthand character or `v-bind:`
    (`:key`, `v-bind:key` and `key` are one name)."""
    m = re.search(r"`([^`]+)`|'([^']+)'|\"([^\"]+)\"", message)
    name = next((g for g in m.groups() if g), None) if m else None
    return re.sub(r"^(?:v-(?:bind|on|slot):|[:@#])", "", name) if name else None


def named_rules(ours):
    """The Sudsy rules whose messages quote a name (their findings compare on it); not the parser's diagnostic, which merges
    the messages of every rule aliased to it and is no rule."""
    return {r for (_, r), ms in ours.items() if r != "syntax-error" and any(message_name(m) for m, _ in ms)}


def target_numbers(tool, per, hit, total, owned, prefix="", wrong=0):
    """`(target share, denominator, out of scope findings)` of a coverage run: see `target_line`."""
    wontfix = {f"{prefix}{e['id']}" for e in catalog_entries(tool, sub=True) if e["coverage"] == "wontfix"}
    out = sum(row[0] for rule, row in per.items() if rule in wontfix)
    left = total + owned - out - wrong
    return (round(1000 * hit / left) / 10 if left else 100.0), left, out


def instruction_end(path, line, continuation):
    """The last line of the instruction that starts on `line` of the file: it goes on while a line ends with the continuation character; comment lines inside it do not end it."""
    lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    end = line
    while end <= len(lines) and lines[end - 1].rstrip().endswith(continuation):
        end += 1
        while end <= len(lines) and lines[end - 1].lstrip().startswith("#"):
            end += 1
    return min(end, len(lines))


def target_line(tool, per, hit, total, owned, prefix="", wrong=0):
    """The target number of a coverage run (docs/ai/design/parity-target.md): `hit` is the findings the mapped rule reports on the
    same line (the column is not compared); the findings of `wontfix` catalog entries (out of scope) and the reviewed `tool-wrong`
    ones (`wrong`, pinned per finding in differences.yaml as `<file> <tool>:<rule> <line>`) leave the denominator; the `owned`
    findings of `formatter` ids stay in it, as missing, until a proof run exists."""
    share, left, out = target_numbers(tool, per, hit, total, owned, prefix, wrong)
    return f"target {share}% ({hit} of {left}, same line); intentional: out of scope (wontfix) {out}, tool-wrong {wrong}, formatter findings counted as missing {owned}"


def run_plain(tool, prefix, work):
    """`(findings, files the tool cannot read)` of ruff, ESLint, typescript-eslint or an ESLint plugin on the corpus folder `work`
    (the ESLint config is written into it); findings are `(file name, rule, message, line)`."""
    if tool == "ruff":
        command, env = parity_format.tool_command(parity_format.tools()["lint"]["ruff"], prefix)
        run = subprocess.run([*command, "."], cwd=work, capture_output=True, text=True, encoding="utf-8", check=False, env=env)
        return [(Path(d["filename"]).name, d["code"], d["message"], d["location"]["row"]) for d in json.loads(run.stdout) if d["code"]], set()
    config = (plugin_config(tool, f'...{PLUGINS[tool]}.configs["flat/recommended"]') if tool in PLUGINS else
              'import js from "@eslint/js";\nexport default [{ linterOptions: { noInlineConfig: true } }, js.configs.recommended];\n' if tool == "eslint" else
              'import tseslint from "typescript-eslint";\nexport default [{ linterOptions: { noInlineConfig: true } }, ...tseslint.configs.recommended];\n')
    (work / "eslint.config.mjs").write_text(config, encoding="utf-8")
    run = subprocess.run(["node", f"{prefix}/node_modules/eslint/bin/eslint.js", "-c", "eslint.config.mjs", "-f", "json", "--no-warn-ignored", "."],
                         cwd=work, capture_output=True, text=True, encoding="utf-8", check=False)
    try:
        results = json.loads(run.stdout)
    except json.JSONDecodeError:
        sys.exit(f"eslint produced no report:\n{run.stdout}\n{run.stderr}")
    fatal = {Path(r["filePath"]).name for r in results if any(m.get("fatal") for m in r["messages"])}
    return [(Path(r["filePath"]).name, m["ruleId"], m["message"], m["line"]) for r in results for m in r["messages"] if m["ruleId"] and not m.get("fatal")], fatal


def proof_plain(tool, prefix, work, sudsy, preset, layout, fatal, before, pinned=lambda f: False):
    """The proof run of a tool that has no `coverage` entry (`parity_coverage.proof` is the other): the corpus folder is copied, formatted with
    `sudsy format` under the tool's preset and checked again; returns the findings of `layout` ids that remain (a file `sudsy format`
    rejects stays as it was, so its findings remain and count)."""
    after = prefix / "proof"
    shutil.rmtree(after, ignore_errors=True)
    shutil.copytree(work, after)
    (after / "sudsy.yaml").write_text(json.dumps({"extends": [f"preset:{preset}"]}), encoding="utf-8")
    run = subprocess.run([sudsy, "format", "."], cwd=after, capture_output=True, text=True, encoding="utf-8", check=False)
    (after / "sudsy.yaml").unlink()
    found, _ = run_plain(tool, prefix, after)
    left = [f for f in found if f[1] in layout and f[0] not in fatal and not pinned(f)]
    per = collections.Counter(f[1] for f in left)
    rejected = sorted({Path(m).name for m in re.findall(r"^error: (.+?): \d+:\d+:", run.stderr, re.M)})
    print(f"proof: `sudsy format` exit {run.returncode}; formatter findings {len(before)} before, {len(left)} after in {len({f[0] for f in left})} files: "
          + (", ".join(f"{r} {n}" for r, n in per.most_common(12)) or "none"))
    print(f"proof: `sudsy format` rejects {len(rejected)} files (Sudsy bugs, counted)" + "".join(f"\n  bug: {n}" for n in rejected))
    return left


def require_corpus(tool, names):
    """Exits 1 with `corpus absent` when the run read no corpus file: a share of 0 of 0 is not a measurement, and the target stays."""
    if names:
        return
    fetch = " ".join(parity_format.tools()["lint"][tool].get("fetch", []))
    print(f"{tool}: corpus absent: no corpus file on this host, nothing measured; target and measured are left as they are" + (f" (fetch it with {fetch})" if fetch else ""))
    raise SystemExit(1)


def rule_maps(tool, spec):
    """`(mapping, aliases)` of a coverage run: upstream rule -> Sudsy rule id, and Sudsy id -> the first id of its entry."""
    mapping = rule_map(tool)
    aliases = rule_aliases(tool)
    if tool == "typescript-eslint":  # its recommended set also sets ESLint core rules (`no-var`), mapped by the eslint catalog
        mapping = {**rule_map("eslint"), **{f"@typescript-eslint/{r}": v for r, v in mapping.items()}}
        aliases = {**rule_aliases("eslint"), **aliases}
    for other in (spec or {}).get("map_also", []):  # the tool runs another tool's checks too (hadolint: shellcheck's on `RUN`)
        mapping = {**rule_map(other), **mapping}
    return mapping, aliases


def sudsy_rules(tool):
    """`(mapped Sudsy rule ids, aliases)`: the rules of the tool's catalog that Sudsy implements, whose findings the landing sample hashes."""
    mapping, aliases = rule_maps(tool, parity_format.tools()["lint"][tool].get("coverage"))
    return set(mapping.values()), aliases


def sudsy_config(tool, spec):
    """The sudsy.yaml of a coverage run: the preset that mirrors the tool's default set and the per-folder settings the tool gets from its flags."""
    presets = parity_format.tools()["lint"][tool]["preset"]
    config = {"extends": [f"preset:{p}" for p in ([presets] if isinstance(presets, str) else presets)]}
    if spec and spec.get("overrides"):
        config["overrides"] = spec["overrides"]
    return config


def work_names(tool, spec):
    """{corpus file: its name in the run's work folder} of a coverage run (an ESLint `.js` is `.mjs` or `.cjs`: both tools read it as a module or a script)."""
    if spec:
        return {p: inside for inside, p in parity_coverage.corpus_files(spec, collections.Counter())}
    out = {}
    for _, name, source in corpus_sources(tool, COVERAGE.get(tool, (None, ()))[1]):
        if tool == "eslint":
            name = str(Path(name).with_suffix(".mjs" if re.search(r"^(?:import|export)\b", source.read_text(encoding="utf-8"), re.M) else ".cjs"))
        out[source] = name
    return out


def name_fn(spec, work):
    """How a diagnostic's path of `sudsy lint` is named in a coverage run: relative to the work folder, or its file name (the plain runs)."""
    return (lambda path: parity_coverage.norm(path, work)) if spec else (lambda path: Path(path).name)


def coverage(tool, sudsy, version, prefix, exe):
    """The share of the findings of `tool`'s default (ruff) or recommended (ESLint, typescript-eslint) set on its
    real-world corpus that the mapped Sudsy rule also reports on the same line of the same file (the column is not
    compared), and by the reported name where the tool's message quotes one and Sudsy's does too. Sudsy runs with its
    default config, as a user gets it with no sudsy.yaml; the share with every mapped rule on is printed beside it.
    Fails when the share is below `coverage:` of tests/parity/<tool>/differences.yaml (a ratchet: it may only go up)."""
    spec = parity_format.tools()["lint"][tool].get("coverage")
    if tool not in COVERAGE and not spec:
        print(f"{tool}: no real-world corpus yet, fixtures only (scripts/parity.sh {tool})")
        return 0
    lang, suffixes = COVERAGE.get(tool, (None, ()))
    mapping, aliases = rule_maps(tool, spec)
    # the rules the formatter owns (the `formatter` catalog entries): `sudsy format` does their work, so they are not findings to cover
    rule_prefix = f"{PLUGINS[tool]}/" if tool in PLUGINS else "@typescript-eslint/" if tool == "typescript-eslint" else ""
    owned_by_format = lambda e: e["sudsy"] == "formatter" or e["sudsy"].startswith("option: ")
    layout = {f"{rule_prefix}{e['id']}" for e in catalog_entries(tool, sub=True) if owned_by_format(e)}
    if tool in PLUGINS:
        mapping = {f"{PLUGINS[tool]}/{r}": v for r, v in mapping.items()}
    folder = ROOT / "tests/parity" / tool
    work = prefix / "coverage"
    shutil.rmtree(work, ignore_errors=True)
    work.mkdir(parents=True)
    presets = parity_format.tools()["lint"][tool]["preset"]  # the preset that mirrors the tool's own default set: the headline (a list when the tool runs another tool, as hadolint runs ShellCheck)
    presets = [presets] if isinstance(presets, str) else presets
    preset = presets[0]
    extends = [f"preset:{p}" for p in presets]
    names = {}
    for source, name in ({} if spec else work_names(tool, spec)).items():
        shutil.copy(source, work / name)
        names[name] = source
    # the default / recommended set of the tool, on every file in one run
    if spec:  # every other tool: the entry says what runs and how its report reads (parity_coverage.py)
        found, count = parity_coverage.findings(tool, spec, prefix, work, exe, {"actionlint": actionlint_check})
        names = dict.fromkeys(range(count))
        fatal = set()
        layout = {e["id"] for e in catalog_entries(tool, sub=True) if owned_by_format(e)}  # layout: `sudsy format` does it
        broken = {f[0] for f in found if f[1] in spec.get("unreadable", [])}  # files the tool cannot read at all (buf: imports the corpus lacks)
        found = [f for f in found if f[0] not in broken]
        fatal = broken
    else:
        found, fatal = run_plain(tool, prefix, work)
    require_corpus(tool, names)
    found = [f for f in found if f[0] not in fatal]
    reviewed = parity_format.Differences(folder, lint=True)  # `<file> <tool>:<rule> <line>`: a finding where the tool is wrong, pinned per finding
    found = leave_out_not_comparable(found, reviewed)
    pinned = lambda f: reviewed.status(f"{f[0]} {tool}:{f[1]} {f[3]}") == "tool-wrong"  # a pinned finding leaves the proof run's count too
    left = [f for f in found if f[1] in layout]  # the layout findings; the proof run below replaces them by those `sudsy format` leaves
    if layout:  # the proof run: the formatter findings that remain after `sudsy format` are the ones that count as missing
        before = [f for f in found if f[1] in layout]
        left = (parity_coverage.proof(tool, spec, prefix, work, exe, {"actionlint": actionlint_check}, sudsy, preset, layout, fatal, before, pinned, reviewed.files) if spec
                else proof_plain(tool, prefix, work, sudsy, preset, layout, fatal, before, pinned))
    owned = len(left)
    leftover = collections.Counter(f[1] for f in left)  # layout rule -> findings left
    places = {}  # rule -> the first places of its missing findings
    found = [f for f in found if f[1] not in layout]
    print(f"input filters: files the tool cannot read {len(fatal)}, formatter-owned findings (counted as missing: {owned}{', after the proof run' if layout else ''})")
    def run_sudsy(config):  # Sudsy's findings with `config` as the sudsy.yaml of the corpus (None: no file, the default a user gets)
        (work / "sudsy.yaml").unlink(missing_ok=True)
        if config is not None:
            if (spec or {}).get("overrides"):  # per-folder settings the tool gets from its own flags (sqlfluff: `--dialect`)
                config = json.dumps({**json.loads(config), "overrides": spec["overrides"]})
            (work / "sudsy.yaml").write_text(config, encoding="utf-8")
        out = subprocess.run([sudsy, "lint", "--format", "json", "."], cwd=work, capture_output=True, text=True, encoding="utf-8", check=False).stdout
        result = {}  # (file, sudsy rule) -> [(message, line)]
        raw[:] = json.loads(out)["diagnostics"]
        for d in raw:
            at = result.setdefault((parity_coverage.norm(d["path"], work) if spec else Path(d["path"]).name, aliases.get(d["rule"], d["rule"])), [])
            at.append((d["message"], d["range"]["start"]["line"]))
            if d["range"]["start"]["offset"] == 0:
                at.append((d["message"], 0))  # a file-level report (it starts at the first character) is also a report at line 0, where a tool that has no line puts its finding
        return result

    examples = {}
    raw, uncovered = [], set()  # the diagnostics of the last run; the files with a finding of the tool that counts and is not covered
    zero = [0, 0, 0]  # findings the tool reports at line 0 (no line): all, those a file-level report covers, those any report in the file has
    filehits = [0]  # the audit: findings with a report of the mapped rule anywhere in the file
    offsets = {}  # upstream rule -> Counter(Sudsy line minus tool line) of the findings reported on another line

    def score(ours):
        """`(per, hits, total, tool-wrong findings by rule)`: a finding is covered when the mapped rule reports it on the same line.
        `per`: upstream rule -> [findings, covered]."""
        named = named_rules(ours)
        per, wrong = {}, collections.Counter()
        instruction = (spec or {}).get("instruction")  # `{"rule_prefix", "continuation"}`: those codes are covered on any line of the instruction the tool's line starts
        offsets.clear()
        uncovered.clear()
        filehits[0] = 0
        zero[:] = [0, 0, 0]
        places.clear()
        for file, rule, _, line in left:
            places.setdefault(rule, []).append(f"{file}:{line}")
            if os.environ.get("COVERAGE_MISSES"):
                print(f"MISS {file} {tool}:{rule} {line} (after `sudsy format`)")
        for file, rule, message, line in found:
            row = per.setdefault(rule, [0, 0])
            row[0] += 1
            target = mapping.get(rule)
            n = message_name(message) if (spec or {}).get("names", True) else None  # `names: false`: the tool's messages quote code, not the name of the finding
            reports = [(m, l) for m, l in ours.get((file, target), []) if target not in named or n is None or n in m] if target else []
            last = instruction_end(work / file, line, instruction["continuation"]) if instruction and rule.startswith(instruction["rule_prefix"]) and line else line
            on_line = any(line <= l <= last for _, l in reports) if line is not None else False  # a finding at line 0 is a file-level one: only a file-level report of the mapped rule covers it
            row[1] += on_line
            zero[0] += line == 0
            zero[1] += line == 0 and on_line
            zero[2] += line == 0 and any(l for _, l in reports)
            filehits[0] += bool(reports)
            if not on_line and os.environ.get("COVERAGE_MISSES"):  # every finding Sudsy does not report on the tool's line: `file rule line`
                print(f"MISS {file} {tool}:{rule} {line}")
            if reports and not on_line and line is not None:  # a report in the file on another line: the offset of the nearest one
                off = min((l - line for _, l in reports if l), key=abs, default=0)
                offsets.setdefault(rule, collections.Counter())[off] += 1
                examples.setdefault((rule, off), f"{file}:{line}")
            is_wrong = not on_line and reviewed.status(f"{file} {tool}:{rule} {line}") == "tool-wrong"
            wrong[rule] += is_wrong
            if not on_line and not is_wrong:
                uncovered.add(file)
            if not on_line and not is_wrong:
                places.setdefault(rule, []).append(f"{file}:{line}")
        total, hit = sum(r[0] for r in per.values()), sum(r[1] for r in per.values())
        return per, hit, total, wrong

    ours = run_sudsy(json.dumps({"extends": extends}))  # the headline: the preset that mirrors the tool's default set
    per, hit, total, wrong = score(ours)
    if os.environ.get("COVERAGE_WRITE") and (spec or tool in COVERAGE):  # the landing sample of the lint leg (scripts/parity_landing.py)
        wanted_lines = {(f, mapping[r], line) for f, r, _, line in found if r in mapping}
        mapped, _ = sudsy_rules(tool)
        state = parity_landing.lint_state(raw, name_fn(spec, work), aliases, mapped)
        corpus = ROOT / "tests/conformance/corpus"
        states, agree = {}, set()
        for source, name in work_names(tool, spec).items():
            if source.is_relative_to(corpus) and name not in fatal:
                rel = source.relative_to(corpus).as_posix()
                states[rel] = state.get(name, set())
                if name not in uncovered and name not in {f[0] for f in left} and all((name, r, line) in wanted_lines for r, line in states[rel]):
                    agree.add(rel)
        parity_landing.write_lint(folder, tool, states, agree)
        if (folder / parity_landing.LINT_FILE).exists():
            parity_landing.prune_lint(folder, tool, parity_format.tools()["lint"][tool], sudsy, prefix / "landing")
    if zero[0]:
        print(f"findings at line 0 (no line): {zero[0]}, covered by a file-level report: {zero[1]}, by any report in the file: {zero[2]}")
    extra = {}  # Sudsy rule -> its findings in files where the tool reports its rule nowhere (a false positive, or a suppression comment)
    wanted = {(f, mapping[r]) for f, r, _, _ in found if r in mapping}
    on = set()  # the Sudsy rules of the rules that the tool's set turns on
    if tool in PLUGINS:
        config = subprocess.run(["node", f"{prefix}/node_modules/eslint/bin/eslint.js", "-c", "eslint.config.mjs", "--print-config", next(iter(names))],
                                cwd=work, capture_output=True, text=True, encoding="utf-8", check=False).stdout
        on = {mapping[r] for r, v in json.loads(config)["rules"].items() if r in mapping and v[0] not in (0, "off")}
    for (f, r), ms in ours.items():
        if r in on and (f, r) not in wanted and f not in fatal:
            extra[r] = extra.get(r, 0) + len(ms)
    print("only Sudsy (files the tool reports the rule in nowhere): " + (", ".join(f"{r} {n}" for r, n in sorted(extra.items(), key=lambda kv: -kv[1])[:12]) or "none"))
    tool_lines = {(f, mapping[r], line) for f, r, _, line in found if r in mapping}  # a finding of Sudsy on a line where the tool reports the mapped rule is no extra
    sudsy_total, sudsy_extra = collections.Counter(), collections.Counter()
    extra_at = collections.defaultdict(list)
    for (f, r), ms in ours.items():
        if r in set(mapping.values()) and f not in fatal:
            for _, line in ms:
                sudsy_total[r] += 1
                sudsy_extra[r] += (f, r, line) not in tool_lines
                if (f, r, line) not in tool_lines:
                    extra_at[r].append(f"{f}:{line}")
    print("only Sudsy on the line (extra findings of the mapped rules): " + (", ".join(f"{r} {n} of {sudsy_total[r]} ({100 * n / sudsy_total[r]:.1f}%)" for r, n in sorted(sudsy_extra.items(), key=lambda kv: -kv[1]) if n) or "none"))
    if os.environ.get("COVERAGE_EXTRAS"):
        Path(os.environ["COVERAGE_EXTRAS"]).write_text(json.dumps(extra_at), encoding="utf-8")
    for r, at in extra_at.items():
        print(f"  extra {r} e.g. " + ", ".join(at[:: max(1, len(at) // 6)][:6]))
    if os.environ.get("COVERAGE_OFFSETS"):
        print(f"  audit: {filehits[0]} of {total} findings have a report of the mapped rule in the file ({round(1000 * filehits[0] / total) / 10 if total else 100.0}%), {hit} of them on the line")  # the line audit: where Sudsy reports a finding the tool puts on another line
        for rule, c in sorted(offsets.items()):
            print(f"  offsets {rule}: " + ", ".join(f"{o:+d} x{n} e.g. {examples[(rule, o)]}" for o, n in c.most_common(4)) + f" (of {per[rule][0] - per[rule][1]} missed)")
    prefixed = "@typescript-eslint/" if tool == "typescript-eslint" else f"{PLUGINS[tool]}/" if tool in PLUGINS else ""
    missed = missed_rules(tool, per, leftover, wrong, prefixed)
    print(f"missing findings, all {sum(missed.values())} by rule:")  # every one is a bug or gap to fix, or a reviewed choice to record
    for rule, n in sorted(missed.items(), key=lambda kv: (-kv[1], kv[0])):
        why = "left after `sudsy format` (formatter or a preset option)" if rule in leftover else f"rule {mapping[rule]} covers {per[rule][1]} of {per[rule][0]}" if rule in mapping else "no Sudsy rule"
        print(f"  {n} {rule}: {why}; e.g. {', '.join(places[rule][:3])}")
    print(target_line(tool, per, hit, total, owned, prefixed, sum(wrong.values())))
    print(f"{tool} {version}: {len(per)} rules in {len(names) - len(fatal)} files, preset:{preset}")
    target = target_numbers(tool, per, hit, total, owned, prefixed, sum(wrong.values()))
    return ratchet(tool, folder, target[0]) | missed_check(tool, folder, per, hit, target[1], leftover, wrong, prefixed)


def leave_out_not_comparable(found, reviewed):
    """`(findings, left out)`: the findings without those of the rules listed under `not-comparable` in differences.yaml (the corpus lacks what
    they need: semantics of code outside it; the evidence per rule is the value), and the line every run prints for them."""
    kept = [f for f in found if f[1] not in reviewed.not_comparable]
    if reviewed.not_comparable:
        count = collections.Counter(f[1] for f in found if f[1] in reviewed.not_comparable)
        print(f"input filters: not comparable on this corpus {len(found) - len(kept)} findings of {len(count)} of {len(reviewed.not_comparable)} listed rules: "
              + ", ".join(f"{r} {n}" for r, n in count.most_common()))
    return kept


def missed_rules(tool, per, leftover=None, wrong=None, prefix=""):
    """Rule -> findings that count as missing in the target: the uncovered ones of each rule (without the `wontfix` rules and the reviewed
    `tool-wrong` findings, which leave the denominator) and the layout findings the proof run leaves (`leftover`)."""
    wontfix = {f"{prefix}{e['id']}" for e in catalog_entries(tool, sub=True) if e["coverage"] == "wontfix"}
    out = collections.Counter(leftover or {})
    for rule, (n, covered) in per.items():
        if rule not in wontfix:
            out[rule] += n - covered - (wrong or {}).get(rule, 0)
    return {r: n for r, n in out.items() if n}


def missed_check(tool, folder, per, hit, total, leftover=None, wrong=None, prefix=""):
    """The `measured:` and `missed:` blocks of the tool's differences.yaml: `missed` is every rule with findings that count as missing (layout
    included), so covered + missed is the denominator of the target; a run where it is not fails. Fails when a recorded rule misses more,
    a rule is missing from the list or the key is absent; says when to lower it. `COVERAGE_WRITE=1` writes both (the queue,
    docs/ai/parity/queue.md, is generated from them)."""
    missed = missed_rules(tool, per, leftover, wrong, prefix)
    if hit + sum(missed.values()) != total:
        print(f"MISSED does not add up: {hit} covered + {sum(missed.values())} missed is not the {total} of the target")
        return 1
    path = folder / "differences.yaml"
    text = path.read_text(encoding="utf-8") if path.exists() else ""
    rows = "".join(f"  {r}: {n}\n" for r, n in sorted(missed.items(), key=lambda kv: (-kv[1], kv[0])))
    if os.environ.get("COVERAGE_WRITE"):
        base = subprocess.run(["git", "merge-base", "HEAD", "origin/main"], cwd=ROOT, capture_output=True, text=True, check=False).stdout.strip()[:8]
        text = re.sub(r"^(?:missed|measured):.*\n(?:[ \t]+.*\n)*", "", text, flags=re.M)
        text += "" if text.endswith("\n") or not text else "\n"
        text += f"measured: {{covered: {hit}, total: {total}, main: '{base}'}}  # of the target; `main`: the origin/main commit the measured tree is based on\n"
        text += ("missed:" if rows else "missed: {}") + "  # rule -> findings that count as missing, layout left after `sudsy format` included; covered + these = total (COVERAGE_WRITE=1 scripts/parity.sh coverage <tool> writes both)\n" + rows
        path.write_text(text, encoding="utf-8", newline="\n")
        print(f"missed: written ({len(missed)} rules, {sum(missed.values())} findings)")
        return 0
    data = yaml.safe_load(text) or {}
    recorded = data.get("missed")
    if recorded is None:
        print(f"no `missed:` in tests/parity/{tool}/differences.yaml: COVERAGE_WRITE=1 scripts/parity.sh coverage {tool} writes it")
        return 1
    bad = 0
    for rule in {**recorded, **(missed if "measured" in data else {})}:  # without `measured` the list is an older one, not complete
        now, most = missed.get(rule, 0), recorded.get(rule, 0)
        if now > most:
            print(f"MISSED above the ratchet: {rule} misses {now} findings > {most}")
            bad = 1
        elif now < most:
            print(f"missed improved: {rule} misses {now}; COVERAGE_WRITE=1 rewrites `missed:`")
    return bad


def ratchet(tool, folder, share, key="target"):
    """Fails when `share` is below the `target` of the tool's differences.yaml; says when to raise it."""
    ratchets = folder / "differences.yaml"
    floor = (yaml.safe_load(ratchets.read_text(encoding="utf-8")) or {}).get(key) if ratchets.exists() else None
    if floor is None or share < floor:
        print(f"{key.upper()} below the ratchet: {share}% < {floor}%" if floor is not None else f"no `{key}:` ratchet in tests/parity/{tool}/differences.yaml: set it to {share}")
        return 1
    if share > floor:
        print(f"{key} improved: raise `{key}:` in tests/parity/{tool}/differences.yaml to {share}")
    return 0


def rubocop_coverage(sudsy, version, prefix):
    """The share of RuboCop's findings (default config, Lint department, Ruby 3.4) on the Ruby corpus
    (tests/conformance/fetch-ruby-corpus.sh) that the mapped Sudsy rule reports on the same line of the same file."""
    also = {"no-assignment-in-condition": "no-assignment-in-logical-condition"}  # Ruby's own rule for the same cop
    mapping = {cop: {rule, also.get(rule, rule)} for cop, rule in rule_map("rubocop").items()}
    folder = ROOT / "tests/parity/rubocop"
    work = prefix / "coverage"
    shutil.rmtree(work, ignore_errors=True)
    shutil.copytree(ROOT / "tests/conformance/corpus/ruby", work)  # the tree, not flat names: a cop's Include and Exclude patterns read the path
    preset = parity_format.tools()["lint"]["rubocop"]["preset"]
    (work / ".rubocop.yml").write_text("AllCops:\n  TargetRubyVersion: 3.4\n  NewCops: disable\n  SuggestExtensions: false\n", encoding="utf-8")
    run = subprocess.run([str(prefix / "bin/rubocop"), "--only", "Lint", "-f", "json", "--cache-root", str(prefix / "cache"), "-P", "."],
                         cwd=work, capture_output=True, text=True, encoding="utf-8", check=False)
    try:
        report = json.loads(run.stdout)
    except json.JSONDecodeError:
        sys.exit(f"rubocop produced no report:\n{run.stdout[:500]}\n{run.stderr[-2000:]}")
    found = [(os.path.normpath(f["path"]), o["cop_name"], o["location"]["start_line"]) for f in report["files"] for o in f["offenses"]]
    def run_sudsy(config):  # Sudsy's findings with `config` as the sudsy.yaml of the corpus (None: no file)
        (work / "sudsy.yaml").unlink(missing_ok=True)
        if config is not None:
            (work / "sudsy.yaml").write_text(config, encoding="utf-8")
        out = subprocess.run([sudsy, "lint", "--format", "json", "."], cwd=work, capture_output=True, text=True, encoding="utf-8", check=False).stdout
        return {(os.path.normpath(d["path"]), d["rule"], d["range"]["start"]["line"]) for d in json.loads(out)["diagnostics"]}

    def score(ours):
        per = {}  # cop -> [findings, covered]
        for file, cop, line in found:
            row = per.setdefault(cop, [0, 0])
            row[0] += 1
            row[1] += any((file, r, line) in ours for r in mapping.get(cop, ()))
        total, hit = sum(r[0] for r in per.values()), sum(r[1] for r in per.values())
        return per, hit, total

    ours = run_sudsy(json.dumps({"extends": [f"preset:{preset}"]}))
    per, hit, total = score(ours)
    for cop, (n, c) in sorted(per.items(), key=lambda kv: kv[1][1] - kv[1][0])[:25]:
        print(f"  {cop}: {c} of {n} covered" + ("" if cop in mapping else " (no Sudsy rule)"))
    wanted = {(f, r, l) for f, c, l in found if c in mapping for r in mapping[c]}
    mapped = set().union(*mapping.values())
    extra = {}  # Sudsy rule -> its findings on lines where RuboCop reports the mapped cop nowhere
    for f, r, l in ours:
        if r in mapped and (f, r, l) not in wanted:
            extra[r] = extra.get(r, 0) + 1
    print("only Sudsy (RuboCop reports the mapped cop nowhere on the line): " + (", ".join(f"{r} {n}" for r, n in sorted(extra.items(), key=lambda kv: -kv[1])[:12]) or "none"))
    print("input filters: none (the corpus is copied whole)")
    print(target_line("rubocop", per, hit, total, 0))
    print(f"rubocop {version}: {len(per)} cops in {len(report['files'])} files, preset:{preset}")
    target = target_numbers("rubocop", per, hit, total, 0)
    return ratchet("rubocop", folder, target[0]) | missed_check("rubocop", folder, per, hit, target[1])


def install_of(config):
    """The install of a tool entry (a name: that of the format entry)."""
    spec = config.get("install", {})
    return parity_format.tools()["format"][spec]["install"] if isinstance(spec, str) else spec


def main():
    tool, sudsy, target = sys.argv[1:4]
    config = parity_format.tools()["lint"][tool]
    version, prefix = parity_format.prepare(config, Path(target).resolve() / "parity")
    if "--coverage" in sys.argv:
        if tool == "rubocop":
            return rubocop_coverage(sudsy, version, prefix)
        return coverage(tool, sudsy, version, prefix, str(parity_format.executable(prefix, install_of(config).get("exe", ""))) if prefix else None)
    if tool == "rubocop":
        print("rubocop: no fixtures, coverage mode only (scripts/parity.sh coverage rubocop)")
        return 0
    if tool == "stylecop":
        return stylecop(sudsy, version)
    if tool == "psscriptanalyzer":
        return psscriptanalyzer(sudsy, version, prefix)
    if tool in ("hadolint", "shellcheck", "checkmake", "actionlint", "buf", "eslint", "typescript-eslint", "stylelint", "stylelint-scss", "stylelint-less", "html-validate", "ruff", *PLUGINS):
        exe = parity_format.tools()["format"][config["install"]] if isinstance(config["install"], str) else config
        return rule_fixtures(tool, sudsy, version, prefix, str(parity_format.executable(prefix, exe["install"].get("exe", ""))))
    if tool not in ("yamllint", "eslint-jsonc", "markdownlint"):
        print(f"{tool}: no fixtures, coverage mode only (scripts/parity.sh coverage {tool})")
        return 0
    mapping = rule_map(tool)
    folder = ROOT / "tests/parity" / tool
    suffixes = {"eslint-jsonc": (".json",), "markdownlint": (".md",)}.get(tool, (".yaml", ".yml"))
    fixtures = sorted(p for p in folder.iterdir() if p.suffix in suffixes and p.name != "sudsy.yaml")
    eslint = {"eslint-jsonc": eslint_jsonc, "markdownlint": markdownlint}[tool](prefix, folder) if tool in ("eslint-jsonc", "markdownlint") else None
    mismatches = 0
    diffs = parity_format.Differences(folder, lint=True)
    for fixture in fixtures:
        found = eslint.get(fixture.name, set()) if eslint is not None else yamllint(fixture, folder, prefix)
        # An upstream rule with no mapping can never match: report it.
        upstream = {(mapping.get(r, f"<unmapped {tool} rule {r}>"), l, c) for r, l, c in found}
        mismatches += compare(fixture.name, upstream, sudsy_lint(sudsy, fixture, folder), tool, diffs)
    diffs.summary()
    print(f"{len(fixtures)} fixtures, {mismatches} mismatch(es) ({tool} {version})")
    return 1 if mismatches else 0


if __name__ == "__main__":
    sys.exit(main())
