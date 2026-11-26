# Design: Gradle evidence

Measurements before any Gradle product code (method: [../development/no-reference-tool.md](../development/no-reference-tool.md), [../architecture/adding-a-language.md](../architecture/adding-a-language.md)). Scope: `*.gradle` (Groovy DSL: build, settings, init, script plugins); `*.gradle.kts` is Kotlin and covered. Scripts: `scripts/evidence_gradle*.py|sh`. Looked up 2026-11-26. Machine: AWS Neoverse-V2 aarch64, 8 cores, rustc 1.99.0. Tools under the slot (`gradle-eval`, `scripts/evidence_gradle_tools.sh`): Temurin JDK 21.0.13, npm-groovy-lint 19.0.1, Gradle 9.9.2, Spotless plugin 8.12.0 (greclipse).

## Corpus
43 projects pinned by commit in `tests/conformance/fetch-gradle-corpus.sh`, listed before any result (known JVM, Android, plugin, Spring, Apache, Netflix, Elastic projects); a project joined if its tree has at least 4 `.gradle` files (of 86 looked up, 43 qualified). 6,481 `.gradle` files (54 MB); `scripts/evidence_gradle.py` caps a project at 60 files (first by path): 1,538 files (3.48 MB). Split by project, seed 20261204: 22 deciding, 21 held out (lists printed by `scripts/evidence.py gradle table`). Groovy's own parser (inside npm-groovy-lint) rejects 2 files (a `$.platforms` GString, a file starting `dependencies {`): the valid set is 1,536.

## Tools (usage evidence)
| Tool | Evidence | Verdict |
|---|---|---|
| Spotless `groovyGradle` + greclipse (Eclipse Groovy formatter) | `com.diffplug.spotless` in 18,048 `build.gradle` (GitHub code search), `groovyGradle` in 1,852, `greclipse` in 1,704; plugin 5.7k stars, pushed 2026-11-25; Apache-2.0; needs JVM and Gradle | primary for format (the languages.yaml estimate named npm-groovy-lint) |
| npm-groovy-lint 19.0.1 (MIT, wraps CodeNarc) | npm 179,900 a month, VS Code extension 491,782 installs; but `.groovylintrc.json` in 241 repositories, 47 workflow and 15 pre-commit hits; 246 stars, pushed 2026-11-25 | second format reference; lint only for generic Groovy |
| CodeNarc (Apache-2.0) | `codenarc` in 878 `build.gradle`; the Gradle plugin lints Groovy source sets, not build scripts | rule engine of npm-groovy-lint |
| Nebula gradle-lint plugin | 316 `build.gradle`, 793 stars; runs inside a build (needs the evaluated project) | partial source, not runnable offline |
| Android lint (Gradle detectors) | runs in every Android build; needs the SDK and a build | partial source (GradleDeprecated, GradleDependency) |
| IntelliJ formatter, prettier-plugin-groovy (npm 21k a month) | editor only; plugin not a reference | none |

## Format
No formatter is shared: on the 1,538 files npm-groovy-lint `--format` (its `format` preset, 4 spaces) changes 1,299 files (84%, 67,155 diff lines; it forces single quotes in 100% of plain strings), greclipse with `indentWithSpaces(4)` changes 1,007 (65%, 71,927 lines; its default indents with tabs), and the two outputs are equal on 322 files (21%). 28 files raised an error in greclipse (Groovy parses all but 2). Setting aside leading blanks, trailing blanks and CRLF, greclipse output equals the source on 1,103 files (71.7%; with blank lines 73.4%): it is mostly an indent normaliser; npm-groovy-lint 417 (527). Neither is held to the corpus style, so the format leg follows the decision table with greclipse as the comparison.

Decision table (`python3 -I scripts/evidence.py gradle table`; one vote per project over the 22 deciding projects; agreement is the share of the 21 held-out projects' occurrences with the chosen form):
| Row | Votes | Rule | Chosen | Held-out agreement |
|---|---|---|---|---|
| indent character | 22 space | default | space | 76.3% (tabs in hibernate-orm and others) |
| indent of a block body | 16 four, 6 two | default + option | 4 spaces, option width | 40.9% (large 2-space projects) |
| quotes of a plain string | 13 single, 9 double | as written | as written | n/a |
| dependency declaration parens | 15 none, 4 parens | default + option; a rewrite, left as written | as written | n/a |
| blank before `{` | 22 one | default | one | 99.4% |
| blanks around `=` | 22 both | default | both | 99.5% |
| blank after `:` in a named argument | 16 after only | default | after only | 88.5% |
| blank after a comma | 21 one | default | one | 98.1% |
| blank after `if/for/while/switch` | 21 one | default | one | 97.9% |
| `else` placement | 14 after `}`, 1 own line | default | after `}` | 100% |
| blanks inside a one-line closure | 18 inside | default | inside | 96.0% |
| blank after `//` | 21 one | default | one | 96.6% |
| blank lines in a row | 22 one | default | one or two kept | 88.2% |
| blank line after `{` | 22 none | default | none | 100% |
| trailing blanks of a file | 22 none | default | none | 99.1% |
| final newline | 22 yes | default | yes | 90.5% |
| line endings | 22 LF | default | LF | 97.8% |
| trailing semicolon | 22 none | default | none | 99.8% |
| continuation indent after a trailing comma | 18 same as previous line | loose measure | as written | n/a |
| line width | 94% of lines at most 80, 1% over 100 | no width evidence | as written | n/a |

Pooled held-out agreement over the decided rows: 87.1% (mean of the rows' project means 92.5%). Indent width is the only row under 80% pooled and it carries the 4-space default with an option. Each row is a measured count on masked code; a row's regex is approximate (the `:` and continuation rows are loose). Not done: the 30-file harm sample (no formatter), meaning oracle (Groovy's parser via npm-groovy-lint gives an AST check: usable, not run), stability (no formatter).

Recommendation: format leg per the no-reference path with greclipse as the comparison tool (its 4-space output is the reference for indent, wrapping and one-line closures), default 4 spaces, option indent width, quotes and parens as written; npm-groovy-lint is not the primary.

## Lint
No tool reads Gradle semantics offline, so the leg follows no-reference-tool.md section 4. npm-groovy-lint default config (CodeNarc `recommended`, 80 s over the corpus): 353 findings of 27 rules in 131 files: JavaIoPackageAccess 210 (EJB-spec rule, noise in build scripts), UnusedVariable 24 (error), UnusedImport 24, CatchException 13, GStringExpressionWithinString 12 (a real bug: `'${x}'`), EmptyMethod 11, others 1 to 7 each; no finding is Gradle-specific. Real and cheap: UnusedImport, UnusedVariable, GStringExpressionWithinString, GStringAsMapKey, DeadCode, EmptyIfStatement, BitwiseOperatorInConditional, ConstantAssertExpression (about 70 findings); the rest is taste or noise (sample of 2 per rule read).

Checklist candidates from the Gradle upgrade guides and the Android plugin API, yield on the corpus (`scripts/evidence_gradle_checks.py`; occurrences / files / projects): `apply plugin:` 869 / 388 / 34; Android `compileSdkVersion` family 481 / 176 / 10; `sourceCompatibility` outside `java {}` 266 / 111 / 25; `buildDir` 145 / 94 / 24; `buildToolsVersion` 136 / 136 / 6; dynamic versions 48 / 30 / 7; `lintOptions` 56 / 56 / 7; removed `compile`/`testCompile` 24 / 11 / 4; `jcenter()` 23 / 12 / 2; `tasks.create` 22 / 19 / 7; archive `baseName`/`classifier` 16 / 14 / 5; `mainClassName` 10 / 8 / 4; `uploadArchives` 1; http:// repository and `<<` tasks 0. Some need the Gradle version and are style (apply plugin, tasks.create). The checklist is not yet reviewed with sources and examples (lead checkpoint), the precision sample is not run.

Built: checklist 85.7% (18 of 21; the three others are `planned`), precision 97.2% (rule list, presets and gaps in [../formatting/gradle.md](../formatting/gradle.md#lint); judged sample `tests/parity/gradle/precision.md`).

Recommendation: lint leg with no reference tool: the checklist above plus the 8 CodeNarc rules that read the file; Android lint and Nebula named as partial sources, left out for needing a build.

## Parser
Decided and measured in [../formatting/gradle.md](../formatting/gradle.md#parser).

## Not verified
Whether the corpus is typical of Kotlin-era repositories that moved to `.kts` (only Groovy files count); greclipse with a project `.eclipse-groovy` properties file (default only); npm-groovy-lint `format` with a custom `.groovylintrc`; slashy-string tokens of the lexer on code with `/` after an identifier in a command expression (135 found, all brackets balanced, not read); first-error causes beyond the top ones; harm sample, stability, meaning oracle, checklist review and precision.
