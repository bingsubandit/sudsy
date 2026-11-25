"""Evidence run of the Dockerfile format (docs/ai/design/dockerfile.md): split, decision table, agreement, tool comparison.

Usage: dockerfile_evidence.py <sudsy-binary> <dockerfmt-binary> <work-dir>

Reads the corpus of fetch-dockerfile-corpus.sh and fetch-dockerfile-evidence-corpus.sh (23 projects), splits the projects
at random (SEED) into deciding and held-out halves, counts the variants of every layout choice per project on the
deciding half (one vote per project), measures how many occurrences of the held-out half already have the form Sudsy
writes, and compares `sudsy format` with `dockerfmt` (equal files, causes of difference, the share that is only shell
code of RUN). The files are only read; the tools get copies in <work-dir>.
"""
import random
import re
import subprocess
import sys
from collections import Counter, defaultdict
from concurrent.futures import ThreadPoolExecutor
from difflib import SequenceMatcher, unified_diff
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CORPUS = ROOT / "tests/conformance/corpus"
SEED = 20261125
# Project of a repository directory when it is not the directory name: the docker-library images and Docker's own
# repositories are one project each, so a project is never on both halves.
PROJECT = {"golang": "docker-library", "python": "docker-library", "nginx": "docker-library", "postgres": "docker-library",
           "buildx": "moby", "docker": "nextcloud", "core": "home-assistant", "docker-node": "nodejs", "dotnet-docker": "dotnet"}
NAMES = re.compile(r"^(Dockerfile(\..*)?|.*\.[Dd]ockerfile)$")
SHELL_KW = {"RUN", "CMD", "ENTRYPOINT"}
# Causes that rewrite syntax instead of laying it out (Sudsy changes only case and blanks)
REWRITES = ("ENV: legacy", "COPY: JSON", "ADD: JSON", "instruction replaced")


def files():
    out = []
    for d in ("dockerfile", "dockerfile-evidence"):
        for f in sorted((CORPUS / d).rglob("*")):
            if f.is_file() and NAMES.match(f.name):
                rel = f.relative_to(CORPUS / d)
                out.append((PROJECT.get(rel.parts[0], rel.parts[0]), f"{d}/{rel}", f))
    return out


def parse(text):
    """Items of a Dockerfile: ('blank', n), ('comment', line), ('ins', record). Independent of the formatter."""
    lines = text.split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    esc = "\\"
    for l in lines:
        m = re.match(r"\s*#\s*escape\s*=\s*(\S)", l, re.I)
        if m:
            esc = m.group(1)
        elif not re.match(r"\s*#\s*\w+\s*=", l):
            break
    items, i = [], 0
    while i < len(lines):
        l = lines[i]
        if not l.strip():
            n = 0
            while i < len(lines) and not lines[i].strip():
                n, i = n + 1, i + 1
            items.append(("blank", n))
        elif l.lstrip().startswith("#"):
            items.append(("comment", l)),
            i += 1
        else:
            phys = [l]
            i += 1
            cont = l.rstrip().endswith(esc)
            while cont and i < len(lines):
                phys.append(lines[i])
                if lines[i].strip() and not lines[i].lstrip().startswith("#"):
                    cont = lines[i].rstrip().endswith(esc)
                i += 1
            heredoc = []
            logical = " ".join(p.rstrip().rstrip(esc) for p in phys if not p.lstrip().startswith("#"))
            for m in re.finditer(r"(?<!<)<<(?!<)-?\s*([\"']?)(\w+)\1", logical):
                end = next((k for k in range(i, len(lines)) if lines[k].strip() == m.group(2)), None)
                if end is not None:
                    heredoc.append(lines[i:end + 1])
                    i = end + 1
            kw = phys[0].split(None, 1)[0] if phys[0].split() else ""
            items.append(("ins", {"kw": kw, "phys": phys, "esc": esc, "heredoc": heredoc, "logical": logical}))
    return items


def variants(path_text):
    """{row: [variant per occurrence]} of one file."""
    text = path_text
    rows = defaultdict(list)
    rows["final newline"].append("yes" if text.endswith("\n") else "no")
    items = parse(text)
    for kind, v in items:
        if kind == "blank":
            rows["blank lines"].append("one" if v == 1 else "several")
        if kind != "ins":
            continue
        kw, phys, esc = v["kw"], v["phys"], v["esc"]
        rows["keyword case"].append("upper" if kw.isupper() else "lower" if kw.islower() else "mixed")
        if kw.upper() == "FROM":
            m = re.search(r"\s(as)\s", v["logical"], re.I)
            if m:
                rows["AS"].append("upper" if m.group(1) == "AS" else "lower" if m.group(1) == "as" else "mixed")
        first = phys[0].rstrip()
        if first.endswith(esc):
            first = first[:-1]
        body = first[len(phys[0]) - len(phys[0].lstrip()) + len(kw):]
        unq = re.sub(r'"[^"]*"|\'[^\']*\'', "Q", body)
        args = unq.strip()
        if kw.upper() in SHELL_KW:
            gaps = [re.match(r"\s*", unq).group(0)]
        else:
            gaps = [re.match(r"\s*", unq).group(0)] + re.findall(r"(?<=\S)(\s+)(?=\S)", args)
        if gaps and gaps[0]:
            rows["blanks between arguments"].append("single" if all(g == " " for g in gaps) else "several")
        j = args.lstrip()
        if j.startswith("[") and kw.upper() != "ONBUILD":
            if re.search(r"\[\s|\s\]", j.replace("Q", "q")) or re.search(r'\[\s|\s\]', body.strip()):
                rows["JSON array"].append("blanks inside brackets")
            elif re.search(r"\",\S|',\S|Q,\S", body):
                rows["JSON array"].append("no blank after comma")
            elif re.search(r"[\"'],\s{2,}|\t", body):
                rows["JSON array"].append("several blanks")
            else:
                rows["JSON array"].append("one blank after comma")
        cont = [p for p in phys[1:] if not p.lstrip().startswith("#") and p.strip()]
        for n, p in enumerate(phys):
            if p.rstrip().endswith(esc) and not p.lstrip().startswith("#"):
                pre = p.rstrip()[:-1]
                gap = len(pre) - len(pre.rstrip(" \t"))
                rows["blanks before backslash"].append("one" if gap == 1 and pre.rstrip() == pre.rstrip(" \t") and not pre.endswith("\t") else "none" if gap == 0 else "several")
        ind = [re.match(r"[ \t]*", p).group(0) for p in cont]
        for p in cont:
            if re.search(r"(&&|\|\|)\s*\\$", p.rstrip()) :
                rows["`&&`, `||` at line end or start"].append("end")
            elif re.match(r"\s*(&&|\|\|)", p):
                rows["`&&`, `||` at line end or start"].append("start")
        for g in ind:
            rows["continuation indent"].append("tab" if "\t" in g else f"{len(g)} spaces")
        for p in phys[1:]:
            if p.lstrip().startswith("#"):
                g = re.match(r"[ \t]*", p).group(0)
                rows["comment in continuation"].append("column 0" if not g else "indented")
        for hl in v["heredoc"]:
            rows["heredoc"].append("as written")
    return rows


def vote(counter):
    return counter.most_common(1)[0][0]


SUDSY = {"keyword case": "upper", "AS": "upper", "blanks between arguments": "single", "JSON array": "one blank after comma",
         "blanks before backslash": "one", "continuation indent": "4 spaces", "comment in continuation": "indented",
         "blank lines": "one", "final newline": "yes", "heredoc": "as written", "`&&`, `||` at line end or start": None}


def decision_table(corpus):
    split = sorted({p for p, _, _ in corpus})
    random.Random(SEED).shuffle(split)
    deciding, held = set(split[: len(split) // 2 + len(split) % 2]), set(split[len(split) // 2 + len(split) % 2:])
    per = {"deciding": defaultdict(lambda: defaultdict(Counter)), "held": defaultdict(lambda: defaultdict(Counter))}
    for proj, name, f in corpus:
        half = "deciding" if proj in deciding else "held"
        try:
            rows = variants(f.read_text(encoding="utf-8"))
        except (UnicodeDecodeError, ValueError):
            continue
        for row, vs in rows.items():
            per[half][row][proj].update(vs)
    return deciding, held, per


def report_table(deciding, held, per):
    print(f"seed {SEED}; deciding ({len(deciding)}): {', '.join(sorted(deciding))}")
    print(f"held out ({len(held)}): {', '.join(sorted(held))}\n")
    print("| row | projects | votes (deciding) | rule | Sudsy writes | agrees | held-out occurrences | held-out with Sudsy form |")
    print("|---|---|---|---|---|---|---|---|")
    tot = [0, 0]
    for row in SUDSY:
        votes = Counter(vote(c) for c in per["deciding"][row].values() if c)
        n = sum(votes.values())
        top, topn = votes.most_common(1)[0] if votes else ("-", 0)
        share = topn / n if n else 0
        rule = "default (>=80%)" if share >= 0.8 else "default + option (60-80%)" if share >= 0.6 else "as written (<60%)"
        occ = Counter()
        for c in per["held"][row].values():
            occ.update(c)
        total, mine = sum(occ.values()), occ[SUDSY[row]]
        if SUDSY[row] is not None:
            tot[0] += mine
            tot[1] += total
        vs = ", ".join(f"{v} {k}" for k, v in votes.most_common(4))
        print(f"| {row} | {n} | {vs} | {rule} | {SUDSY[row] or 'as written'} | {'-' if SUDSY[row] is None else 'yes' if top == SUDSY[row] and share >= 0.6 else 'no'} | {total} | {mine} ({100 * mine / max(total, 1):.1f}%) |")
    print(f"\nagreement (held out, pooled over the rows above): {tot[0]}/{tot[1]} = {100 * tot[0] / max(tot[1], 1):.1f}%")
    print("\nvariant counts per project, deciding half:")
    for row in SUDSY:
        print(f"- {row}:")
        for p, c in sorted(per["deciding"][row].items()):
            print(f"    {p}: {dict(c)}")
    print("\nvariant counts, held-out half (occurrences):")
    for row in SUDSY:
        occ = Counter()
        for c in per["held"][row].values():
            occ.update(c)
        print(f"- {row}: {dict(occ)}")


def run_sudsy(sudsy, work, corpus):
    def one(arg):
        i, (_, name, f) = arg
        d = work / "sudsy" / str(i)
        d.mkdir(parents=True, exist_ok=True)
        (d / f.name).write_bytes(f.read_bytes())
        r = subprocess.run([sudsy, "format", f.name], cwd=d, capture_output=True, text=True, encoding="utf-8", check=False)
        return None if r.returncode else (d / f.name).read_text(encoding="utf-8")
    with ThreadPoolExecutor(8) as ex:
        return list(ex.map(one, enumerate(corpus)))


def run_dockerfmt(dockerfmt, work, corpus):
    (work / "df").mkdir(parents=True, exist_ok=True)
    def one(arg):
        _, (_, name, f) = arg
        try:
            r = subprocess.run([dockerfmt], cwd=work / "df", input=f.read_bytes(), capture_output=True, check=False)
        except OSError:
            return None
        return None if r.returncode else r.stdout.decode("utf-8", "replace")
    with ThreadPoolExecutor(8) as ex:
        return list(ex.map(one, enumerate(corpus)))


def norm(s):
    return re.sub(r"\s+", " ", re.sub(r"\\\n", " ", s)).strip()


def compact(s):
    return re.sub(r"\s+", "", s.replace("\\", ""))


def shell_ins(v):
    if v["kw"].upper() == "HEALTHCHECK":
        return bool(re.search(r"\bCMD\s+[^\[\s]", v["logical"]))
    return v["kw"].upper() in SHELL_KW and not re.match(r"\s*\w+\s+\[", v["logical"])


def item_text(it):
    if it[0] == "ins":
        return "\n".join(it[1]["phys"] + [l for h in it[1]["heredoc"] for l in h])
    return it[1] if it[0] == "comment" else ""


def causes(a, b):
    """[(cause, is_shell)] of the differences between two outputs a (Sudsy) and b (dockerfmt); items aligned by content."""
    ia, ib = [x for x in parse(a) if x[0] != "blank"], [x for x in parse(b) if x[0] != "blank"]
    out = []
    ba, bb = [x[1] for x in parse(a) if x[0] == "blank"], [x[1] for x in parse(b) if x[0] == "blank"]
    if ba != bb:
        out.append(("blank lines between instructions", False))
    ka = [compact(item_text(x)).lower() for x in ia]
    kb = [compact(item_text(x)).lower() for x in ib]
    sm = SequenceMatcher(None, ka, kb, autojunk=False)
    for tag, i1, i2, j1, j2 in sm.get_opcodes():
        if tag == "equal":
            out += [classify(x, y) for x, y in zip(ia[i1:i2], ib[j1:j2]) if item_text(x) != item_text(y)]
            continue
        pairs = list(zip(ia[i1:i2], ib[j1:j2])) if (i2 - i1) == (j2 - j1) else []
        if not pairs:
            for x in ia[i1:i2] + ib[j1:j2]:
                out.append(("instruction added/removed/rewritten (%s)" % (x[1]["kw"].upper() if x[0] == "ins" else x[0]), x[0] == "ins" and shell_ins(x[1])))
            continue
        for x, y in pairs:
            out.append(classify(x, y))
    return out


EXAMPLES = {}


def classify(x, y):
    c = classify1(x, y)
    EXAMPLES.setdefault(c[0], []).append((item_text(x), item_text(y)))
    return c


def classify1(x, y):
    if x[0] == "comment" or y[0] == "comment":
        return ("comment: blank after `#`, indent", False)
    vx, vy = x[1], y[1]
    sh = shell_ins(vx) or shell_ins(vy)
    kw = vy["kw"].upper()
    if vx["kw"].upper() != kw:
        return (f"instruction replaced ({vx['kw'].upper()} -> {kw})", False)
    if sh:
        la, lb = [p.strip() for p in vx["phys"]], [p.strip() for p in vy["phys"]]
        if compact(vx["logical"]) == compact(vy["logical"]):
            if la == lb:
                return (f"{kw}: indent of continued lines", True)
            if any(l.endswith("&& \\") for l in la) and any(l.startswith("&&") for l in lb):
                return (f"{kw}: `&&` and `||` moved from line end to line start", True)
            return (f"{kw}: lines joined or split", True)
        if re.sub(r"[;&|<>\\\s]", "", vx["logical"]) == re.sub(r"[;&|<>\\\s]", "", vy["logical"]):
            return (f"{kw}: shell separators, redirect blanks, case layout", True)
        return (f"{kw}: shell text rewritten (quotes, heredoc, comments)", True)
    if norm(vx["logical"]) == norm(vy["logical"]):
        return (f"{kw}: continuation, indent or blank layout", False)
    if norm(vx["logical"]).lower() == norm(vy["logical"]).lower():
        return (f"{kw}: keyword or stage name case", False)
    if kw == "ENV" and re.match(r"ENV\s+[^\s=]+\s+\S", vx["logical"].strip(), re.I) and "=" not in vx["logical"].split()[1]:
        return ("ENV: legacy `ENV key value` written as `key=value`", False)
    if kw in ("COPY", "ADD") and "[" in vx["logical"] and "[" not in vy["logical"]:
        return (f"{kw}: JSON array written as plain arguments", False)
    if kw in ("ENV", "ARG", "LABEL"):
        return (f"{kw}: quoting or pair layout", False)
    return (f"{kw}: argument text rewritten", False)


# Gap names of tests/parity/dockerfmt/differences.yaml, in the order that picks the first cause of a differing file.
GAPS = (("comment-space", "comment:"), ("blank-lines", "blank lines"), ("copy-layout", ": continuation, indent or blank"),
        ("maintainer", "MAINTAINER"), ("env-legacy", "ENV: legacy"), ("copy-json", "JSON array"))


def first_cause(cs):
    for name, key in GAPS:
        if any(key in c for c, _ in cs):
            return name
    return "shell-layout" if all(sh for _, sh in cs) else "other"


def compare(corpus, held, sud, df):
    equal, unchanged = 0, 0
    nfiles = 0
    causes_all, files_by_cause = Counter(), defaultdict(set)
    shell_only, diff_files, rewrite_only = 0, 0, 0
    harm = []
    for (proj, name, f), s, d in zip(corpus, sud, df):
        if s is None or d is None:
            continue
        nfiles += 1
        if s == d:
            equal += 1
            continue
        diff_files += 1
        cs = causes(s, d)
        fine = [c for c in cs if not c[1]]
        if not fine:
            shell_only += 1
        elif all(c[0].startswith(REWRITES) for c in fine):
            rewrite_only += 1
        FIRST[(name in HELD_NAMES, first_cause(cs))] += 1
        for c, sh in cs:
            causes_all[(c, sh)] += 1
            files_by_cause[(c, sh)].add(name)
        harm.append(name)
    return nfiles, equal, diff_files, shell_only, causes_all, files_by_cause, rewrite_only


FIRST = Counter()
HELD_NAMES = set()


def main():
    sudsy, dockerfmt, work = sys.argv[1], sys.argv[2], Path(sys.argv[3])
    corpus = files()
    projects = {p for p, _, _ in corpus}
    print(f"{len(corpus)} files, {len({n.split('/')[1] for _, n, _ in corpus})} repositories, {len(projects)} projects\n")
    deciding, held, per = decision_table(corpus)
    report_table(deciding, held, per)
    sud, df = run_sudsy(sudsy, work, corpus), run_dockerfmt(dockerfmt, work, corpus)
    print(f"\nSudsy failed on {sum(s is None for s in sud)} files, dockerfmt on {sum(d is None for d in df)}")
    for (p, n, _), s, d in zip(corpus, sud, df):
        if s is None or d is None:
            print(f"  failed: {n} ({'Sudsy' if s is None else 'dockerfmt'})")
    HELD_NAMES.update(n for p, n, _ in corpus if p in held)
    ok = [(c, s, d) for c, s, d in zip(corpus, sud, df) if s is not None and d is not None]
    for label, who in (("held out", lambda p: p in held), ("all", lambda p: True)):
        sub = [(c, s, d) for c, s, d in ok if who(c[0])]
        su = sum(s == c[2].read_text(encoding="utf-8") for c, s, _ in sub)
        du = sum(d == c[2].read_text(encoding="utf-8") for c, _, d in sub)
        print(f"[{label}] files left byte-equal by Sudsy: {su}/{len(sub)} ({100 * su / max(len(sub), 1):.1f}%), by dockerfmt: {du}/{len(sub)} ({100 * du / max(len(sub), 1):.1f}%)")
    changed = [(c, s) for c, s, _ in ok if s != c[2].read_text(encoding="utf-8")]
    sample = random.Random(SEED + 1).sample(changed, min(30, len(changed)))
    print(f"\nsample of {len(sample)} of {len(changed)} files Sudsy changes (diff, first 10 lines each):")
    for (p, n, f), s in sample:
        diff = list(unified_diff(f.read_text(encoding="utf-8").splitlines(), s.splitlines(), "source", "sudsy", lineterm="", n=0))
        print(f"=== {n}: {len(diff)} diff lines")
        print("\n".join(l[:150] for l in diff[2:12]))
    FIRST.clear()
    for label, sel in (("all", lambda p: True), ("held out", lambda p: p in held), ("deciding", lambda p: p in deciding)):
        sub = [(c, s, d) for c, s, d in zip(corpus, sud, df) if sel(c[0])]
        n, eq, dif, sh, ca, fb, rw = compare([c for c, _, _ in sub], held, [s for _, s, _ in sub], [d for _, _, d in sub])
        print(f"\n[{label}] both succeed: {n}; equal: {eq} ({100 * eq / max(n, 1):.1f}%); differ only in shell code of RUN/CMD/ENTRYPOINT: {sh} ({100 * sh / max(n, 1):.1f}%); "
              f"equal if shell left out: {eq + sh} ({100 * (eq + sh) / max(n, 1):.1f}%); "
              f"also left out the rewrites (legacy ENV, COPY JSON, MAINTAINER): {eq + sh + rw} ({100 * (eq + sh + rw) / max(n, 1):.1f}%)")
        if label == "all":
            print("first cause per half (deciding, held-out):", {k: (FIRST[(False, k)], FIRST[(True, k)]) for k in sorted({k for _, k in FIRST})})
            if len(sys.argv) > 4:
                for e in EXAMPLES.get(sys.argv[4], [])[:int(sys.argv[5]) if len(sys.argv) > 5 else 3]:
                    print("--- sudsy\n" + e[0] + "\n--- dockerfmt\n" + e[1])
            for (c, shell), k in ca.most_common(15):
                if len(sys.argv) > 4 and sys.argv[4] == c:
                    print("   files: " + ", ".join(sorted(fb[(c, shell)])[:8]))
                print(f"  {k:6d} {len(fb[(c, shell)]):4d} files  {'[shell] ' if shell else ''}{c}")


if __name__ == "__main__":
    main()
