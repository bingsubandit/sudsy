# Helm templates (`lang-helm`)

## Parser
tree-sitter through `core-treesitter` (mapping in `crates/lang-helm/lang.yaml`, grammar `grammars/helm.grammar`) over `grammars/tree-sitter-gotmpl`, a copy of ngalaiko/tree-sitter-go-template (`gotmpl`, ABI 15; [FORK.md](../../../grammars/tree-sitter-gotmpl/FORK.md)). The user chose it on the measurements of [../design/helm.md](../design/helm.md). The text between actions is one `Text` token per run (the host YAML, read as embedded regions by the lint leg).
Files: `*.yaml`, `*.yml` and `*.tpl` anywhere below a folder named `templates` (`file-names` pattern `templates/**/*.ext`); the more specific pattern wins over the YAML language.

## Formatting
`core_format::template_layout` (shared) over the actions; the text between actions is printed as written, because blank lines and trailing blanks of the host YAML can be the content of a block scalar.
- One blank inside the delimiters of a single-line action (`{{ x }}`, `{{- x -}}`); comments and actions over several lines stay as written.
- An own-line action with a left trim (`{{- if }}`, `{{- else }}`, `{{- end }}`) stands at the indent of the next line that has content (not blank, not a comment or an opening block action alone on its line); its `else` and `end` stand where their opener stands. `define` stands where it is. The trim removes the blanks before the action, so the produced text is the same. Option `align_control_actions` (default on) turns it off.
- Trims, `indent`/`nindent` filters, the YAML indent step, blank lines inside the text and trailing blanks are not changed (the first two change the produced text, the rest may be block scalar content). The engine trims blank lines and blanks at the end of the file and writes one final newline.

## Options
<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
| `align_control_actions` | bool | `true` | Own-line control actions with a left trim (`{{- if }}`, `{{- else }}`, `{{- end }}`) stand at the indent of the content they open; `else` and `end` stand where their opener stands. The trim removes the blanks in front of the action, so the output text does not change. |
<!-- /generated -->

## Measured
Corpus of 3,018 template files (`tests/conformance/fetch-helm-corpus.sh`): 3 do not parse (invalid Go templates: a multi-character quoted string, `.Values.a-b`, a missing `.`); the others pass round trip, meaning check and idempotency (`scripts/parity.sh stability helm`: 3,015 stable of 3,018). Held-out agreement of the rows the formatter writes: 95.7% pooled ([../design/helm.md](../design/helm.md)). `sudsy format --check` on the corpus: 0.29 s ([../performance/numbers.md](../performance/numbers.md)).
