# GitHub Actions (`lang-github-actions`)

Workflows (`.github/workflows/*.yml`, `*.yaml`) and action metadata (`action.yml`, `action.yaml`) are YAML with their own rules and embedded scripts.

## Selection and provider
A separate provider, `github-actions`, with the YAML grammar: it claims files by path (a `yml` or `yaml` file directly inside a folder named `workflows`, a file named `action.yml` or `action.yaml`; the folder name is not checked against `.github`, the matcher knows one directory name), which beat the extension matcher of `yaml`. Parsing, tree, formatter, hooks and option schema are `lang-yaml`'s, called through the provider (no copy of the grammar, no second parser). Its config base is `yaml` ([../config/sudsy-yaml.md](../config/sudsy-yaml.md#dialects)): the `yaml:` section's options and rule levels apply to workflows and actions, `github-actions:` on top, and every `yaml` rule (pattern rules included) runs besides the rules below. It has no presets of its own.

## Embedded scripts
The `run:` of every step of a workflow's jobs and of a composite action is code of another language ([../architecture/adding-a-language.md](../architecture/adding-a-language.md#embedded-languages)), found by the key-path spec in `src/scripts.rs` (facts: paths, shell words, the carved expression) read by `core_rules::embed_keys`, which also gives the two script rules their text (a region with no language is text no parser reads: a `>` block, an unknown shell, the JavaScript of `actions/github-script`):
- Which language: the step's `shell:`, else the job's or the workflow's `defaults.run.shell`, else PowerShell on a `runs-on` that names `windows`, else Shell. `bash` and `sh` (with options after the name) are Shell, `pwsh` and `powershell` PowerShell; anything else (`python`, `cmd`, a custom template) is not checked.
- Which text: a single-line plain scalar or a `|` block. A `>` block, a quoted scalar and a multi-line plain scalar fold or escape, so they are not code as written. The block's indentation and every `${{ }}` expression are carved out (blanked to spaces for the parse), so the Shell rules see `echo  ` for `echo ${{ github.sha }}` and report at workflow positions. A script with a here-document is left out when its terminator is indented (the parse would not find the terminator).
- Formatting never touches the script (`format: false`); a workflow's indentation is the YAML formatter's.

## Rules
| Rule | Does |
|---|---|
| `invalid-expression` | Syntax of `${{ }}`, parsed by the shared `core_rules::expr` (grammar, type language as text, tables of contexts, functions and per-key availability in `src/expression.rs`): unclosed, empty, brackets, strings, stray characters, unknown context or function. |
| `expression-context` | Off by default (preset `github-actions/actionlint`). A context or special function the key does not offer (`env` in a job `env`, `runner` in a job `if`). |
| `expression-types` | Off by default (preset). A property a typed context lacks, a wrong number or type of arguments, a `format` placeholder with no argument, an object interpolated into text. `steps` (ids before the step, outputs untyped), `needs`, `matrix` and `inputs` take their type from the file itself (`Scope` rows: key paths and key names; `core_rules::expr::scope`). |
| `if-condition` | Off by default (preset). An `if:` that is a constant, or has text (a space, a newline of a `\|` block, `&&`) around its `${{ }}` and so is always true. |
| `invalid-glob` | Off by default (preset). A branch, tag or path filter that is not valid: a bad range, escape or quantifier, a character a ref name cannot have, `.` or `..` or edge spaces in a path (shared `Format::RefGlob`/`PathGlob`). |
| `outdated-action` | Off by default (preset). `uses:` of a popular action at a spec whose runner is retired (exact `owner/repo@ref`) (the list `data/outdated-actions.txt` of actionlint's release, read by the shared `Format::Outdated`). |
| `script-injection` | An attacker-controlled context (issue and pull request titles and bodies, branch names, commit messages, ...) pasted into `run:` or the `script` of `actions/github-script`. |
| `deprecated-commands` | `::set-output`, `::save-state`, `::set-env`, `::add-path` in such a script. |
| `invalid-job-id` | Characters of job ids. |
| `duplicate-ids` | Job ids that differ only in case, repeated step ids of a job. |
| `unknown-needs`, `cyclic-needs` | `needs` of an unknown job; one report at the first job of each cycle. |
| `invalid-runs-on` | A label not matched by the `labels` option (regular expressions; default: the hosted images and `self-hosted`); skipped with `self-hosted`, an expression, or a `group`. |
| `workflow-syntax` | Off by default (the preset `github-actions/actionlint` turns it on): matrix duplicates and excludes that match nothing (shared `shape::combos`), step id characters, a service or container password that is not an expression; the structure of a workflow: keys a section does not have, required keys, kinds, empty sections, repeated keys of `env`, `with`, `matrix`, ... Rows in `src/shape.rs` (key paths, facts) read by the shared `core_rules::shape` walk. |
| `invalid-event` | Off by default: the `type`, `default` and `options` of `workflow_dispatch` and `workflow_call` inputs (shared `shape::typed`),  event names, the keys and activity types of each event, a filter with its `-ignore` twin, a filter an event does not have (`scoped` in `shape.rs`), cron lines of `schedule` with the shortest interval (5 minutes) and the `timezone` (the list `data/timezones.txt`, shared `Format::Listed`), the keys of `workflow_dispatch` and `workflow_call` inputs. |
| `invalid-permissions` | Off by default: a scope or value `permissions` does not have. |
| `missing-permissions` | Off by default: a job without `permissions` in a workflow without top-level `permissions`. |

Checked against actionlint by `scripts/parity.sh actionlint` ([../development/parity.md](../development/parity.md#actionlint)); the catalog [../parity/actionlint.yaml](../parity/actionlint.yaml) lists what is not done. Not done, offline: the types of step outputs, the maximum of 25 dispatch inputs, shell names per OS, runner label conflicts. Composite actions (`.github/actions/*/action.yaml`) are not workflows: actionlint's 13 findings when the corpus feeds it those files stay uncovered (`action-files` in `differences.yaml`). Not done, needing the network or other files: the inputs and outputs of the actions a step uses, local actions and reusable workflows.


## Options
<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `line_width` | int 20-1000 | `80` | Preferred maximum line width in columns. |
| `indent_size` | int 1-16 | `2` | Columns per indentation level (also the width of a tab). |
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
| `bracket_spacing` | bool | `true` | Spaces inside the braces of a one-line mapping or object: `{ a: 1 }` or `{a: 1}`. |
| `array_bracket_spacing` | bool | `false` | Spaces inside the brackets of a one-line sequence or array: `[ 1, 2 ]` or `[1, 2]`. |
| `sequence_indent` | `preserve`, `indented`, `flush` | `preserve` | A block sequence that is the value of a mapping key: `indented` puts the dashes one level in, `flush` at the key's column, `preserve` keeps each sequence as written. |
| `quote_style` | `preserve`, `single`, `double`, `prefer_double` | `preserve` | Quote style of quoted scalars. With `single` and `double` a scalar keeps its quotes when the other style would change its value or need more escapes; `prefer_double` uses double quotes, except single quotes for text that contains a `"` (unless it has other escapes, which keep it as written). |
| `document_start` | `preserve`, `always`, `never` | `preserve` | The `---` marker: `always` starts every document with it, `never` leaves it out of the first document when that is possible, `preserve` keeps what is written. |
| `document_end` | `preserve`, `always`, `never` | `preserve` | The `...` marker: `always` ends every document with it, `never` leaves it out unless the next document has a directive or no `---`, or the document is empty, `preserve` keeps what is written. |
| `max_blank_lines` | int 0-10 | `1` | Largest number of blank lines kept between entries, comments and documents. |
| `blank_lines_after_key` | `preserve`, `remove` | `preserve` | Blank lines between a key and its block value on the next line: `preserve` keeps them (up to `max_blank_lines`); `remove` drops them. |
| `comment_spacing` | int 1-10 | `1` | Spaces between a value and a comment after it on the same line. |
| `comment_indent` | `preserve`, `content` | `preserve` | A comment on its own line after the end of a block collection: `preserve` keeps it with the outermost collection it is indented as deep as, `content` with the innermost one when it is deeper than the next token (so it is indented like the line before it or like the next token). |
<!-- /generated -->

## Conformance
`tests/conformance/fetch-github-actions-corpus.sh` (actionlint's test workflows at v1.7.12, `actions/starter-workflows`), `cargo test --profile corpus -p lang-github-actions --test integration -- --ignored conformance`: every file that parses formats with every check.
