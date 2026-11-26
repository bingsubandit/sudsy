# Design: Helm template evidence

Measurements before any Helm product code (method: [../development/no-reference-tool.md](../development/no-reference-tool.md), [../architecture/adding-a-language.md](../architecture/adding-a-language.md)). Scope: `*.yaml`, `*.yml` and `*.tpl` under a `templates` folder of a chart: Go templates (text/template) in YAML. Scripts: `scripts/evidence_helm*.py|sh`, `scripts/evidence_helm_parsers/`, `tests/conformance/fetch-helm-corpus.sh`. Looked up 2026-11-26. Machine: AWS Neoverse-V2 aarch64, 8 cores. Tools in the slot (`/scratch/slots/sudsy/3`): helm 3.16.4, kubeconform 0.6.7, prettier 3.4.2 (npm), Python 3.12.

## Corpus
3,018 template files (`.yaml`, `.yml`, `.tpl`) in the fetched trees, 2,317 from 19 named chart repositories and 701 from batch 2. The evidence reads 1,389 files in 603 projects (cap: 40 files per project, the first by path, `scripts/evidence_helm.py`).
- Batch 1, 19 chart repositories pinned by commit (default-branch heads of 2026-11-26, `scripts/evidence_helm_pins.sh`): prometheus-community/helm-charts, grafana/helm-charts, kubernetes/ingress-nginx, argoproj/argo-helm, jetstack/cert-manager, traefik/traefik-helm-chart, elastic/helm-charts, open-telemetry/opentelemetry-helm-charts, kyverno/kyverno, external-secrets/external-secrets, hashicorp/vault-helm, DataDog/helm-charts, nginxinc/kubernetes-ingress, rook/rook, redpanda-data/helm-charts, apache/airflow, temporalio/helm-charts, zalando/postgres-operator, cilium/cilium. Dropped by the threshold (3 templates): goharbor/harbor-helm (1), kubernetes/dashboard (1). The list was written before any file was read. Batch 1 also fetches `Chart.yaml` and `values.yaml` of the same trees (the lint and render inputs; 118 chart folders after subcharts are left out).
- Batch 2, 1,000 blobs: the first 1,000 items of the GitHub code search `"{{ .Values" path:templates` (31,232 hits; relevance order; `tests/conformance/helm-search.tsv`, pinned by blob sha, written before any file was read). The phrase keeps Go templates of charts and leaves out other `{{ }}` templates (cookiecutter, Ansible). 723 repositories; 46 `.tpl` files. Batch 2 blobs are single files: no `Chart.yaml`, so they are not a lint or render input.
- Split by project, seed 20261204: deciding 302 projects (595 files), held out 301 projects (794 files) (`python3 -I scripts/evidence.py helm list`).

## Tools
Usage evidence (`scripts/evidence_helm_usage.sh`, code search totals and registry lookups, 2026-11-26):

| Tool | Evidence | Verdict |
|---|---|---|
| helm lint (official, Go) | 13,264 code search hits in workflows (`"helm lint"`); 274 in pre-commit configs | the standard check of a chart; renders the chart; not a template formatter or linter for the layout |
| helm template + kubeconform | `helm template` in workflows 9,136; kubeconform 6,064 | the validity check of the rendered output (see Lint) |
| chart-testing (`ct lint`) | 3,816 (`chart-testing`), 4,400 (`ct lint`) in workflows | runs helm lint and the chart checks; not a formatter |
| helm-docs | 741 in pre-commit configs | generates README from values; not a formatter |
| prettier, yamllint in workflows | 61,568 and 40,640 (not Helm-specific) | run on the YAML of a repository; see the prettier result below |
| PyPI `helm-lint`, `helm-format`, `helmfmt`, `chart-testing` | 404 (not found) | none |
| npm `prettier-plugin-go-template` | 0.0.15, last modified 2023-07-20 | not read; a prettier plugin for Go templates, not a Helm formatter |
| npm `prettier-plugin-helm`, `helm-language-server`, `helm-lint`, `eslint-plugin-helm`, `tree-sitter-go-template` | not found | none |

**Helm has no formatter in use**: no maintained formatter for Helm templates was found (the search and the registries above). `helm lint` is the only tool that chart repositories run on templates, and it is a validity check, not a layout tool.

Prettier 3.4.2 on the 3,018 template files (`scripts/evidence_helm_tools.py`, the yaml parser on stdin): parse error 2,903 (96.2%), changed 107 (3.5%), unchanged 8 (0.3%). Go actions in YAML are not YAML, so a YAML formatter cannot be the Helm leg.

Helm tools on the 19 batch 1 repositories (`scripts/evidence_helm_tools.py`; helm 3.16.4; 118 chart folders):
- `helm lint`: exit 0 on 107 of 118 folders; 13 `[ERROR]` lines and 16 `[WARNING]` lines in all (the messages are not classified).
- `helm template` (default values) renders 93 of 118. The 25 that do not render: 12 with dependencies not vendored (`charts/` missing a `Chart.yaml` dependency; `helm dependency build` needs the network), 6 that deprecation-warn (`WARNING: This chart is deprecated` makes `helm template` exit non-zero), 6 that require a value (`required` in a template: `clusterAgent.serviceAccount`, the `tokenSecretName`, the postgres password, the store driver, `image.repository`, the searcher pod size), 1 library chart (`library charts are not installable`).
- kubeconform 0.6.7 with Kubernetes 1.31.0 schemas, missing schemas ignored, on the 93 rendered charts: 558 valid resources, 0 invalid, 0 errors, 45 skipped (CRDs, no schema).

## Format
No Helm formatter exists. Decision table (`python3 -I scripts/evidence.py helm table`): one vote per project on the deciding half; agreement is the share of the held-out occurrences with the form Sudsy would write. Trims and the `indent`/`nindent` filter change the rendered text, and a control action's position is part of the YAML text: these rows stay as written, whatever the vote.

| Row | Votes (deciding projects) | Rule | Sudsy writes | Held-out agreement |
|---|---|---|---|---|
| indent of a control line against the content it opens | 224: 199 same, 22 shallower, 3 deeper | default | same | 71.3% (4,306 of 6,041) |
| indent of an end line against its opener | 224: 224 same | default | same | 99.6% (6,308 of 6,334) |
| control action on its own line | 225: 222 own line, 3 shared | default | as written | n/a |
| left trim of a control action (`{{-`) | 225: 215 trim, 10 plain | default | as written | n/a (changes the output) |
| right trim of a control action (`-}}`) | 225: 215 plain, 10 trim | default | as written | n/a (changes the output) |
| space after `{{` of an action | 302: 300 one, 2 none | default | one | 98.5% (27,196 of 27,610) |
| space before `}}` of an action | 302: 299 one, 3 none | default | one | 97.3% (26,859 of 27,610) |
| indent filter (`indent` or `nindent`) | 207: 176 nindent, 31 indent | default | as written | n/a (changes the output) |
| YAML indent step of a host line | 293: 289 two, 4 other | default | two | 98.8% (725 of 734) |
| tab in the indent of a line | 299: 299 space | default | space | 100.0% (111,351 of 111,351) |
| blank lines in a row | 77: 76 one, 1 two | default | one | 98.6% (1,603 of 1,626) |
| blank lines at the start of a file | 302: 302 none | default | none | 99.7% (792 of 794) |
| blank lines at the end of a file | 302: 291 none, 11 present | default | none | 97.4% (773 of 794) |
| trailing blanks of a line | 302: 302 none | default | none | 100.0% (125,654 of 125,709) |
| final newline | 302: 247 yes, 55 no | default | yes | 89.5% (711 of 794) |
| line endings | 302: 299 LF, 3 CRLF | default | LF | 99.7% (792 of 794) |

Pooled held-out agreement over the rows Sudsy writes: 307,070 of 310,191 = 99.0% (mean of the rows' project means 95.5%). The weakest row is the indent of a control line (71%: the deciding vote is 89% and the held-out occurrences are spread): that row is a default with a second form needed; a formatter writes `same` only with an option for the other.

Observations for the decision: the `{{- end }}` lines sit at the indent of their opener in 99.6% of the held-out occurrences; trims are used by 96% of the projects, which is why a formatter must keep them as written.

## Lint
Helm's own validity checks (above) are the reference: `helm lint` 107 of 118 exit 0; kubeconform 0 invalid of 558 rendered resources. No lint checklist was run (no Helm lint rules of ours; a checklist of the chart rules of `helm lint` would replace the tool's own messages).

## Parser candidates
Measured with `scripts/evidence_helm_parsers` (one binary per candidate; release, LTO, one codegen unit, strip; the 1,389 evidence files of `evidence.py helm list`; best of 5 whole-corpus parses from memory, warm; 16 probes in `scripts/evidence_helm_parsers/probes`; mutants: the middle `}}` of each file removed, 1,387 files with a `}}`). Commands: `bash scripts/evidence_helm_parsers/fetch-grammar.sh`, then `cargo build --release --manifest-path scripts/evidence_helm_parsers/Cargo.toml --features ts --bin base --bin ts --bin hand`.

| Row | tree-sitter-go-template 06b2fd2 (grammar 0.19.5, `gotmpl`, ABI 15; runtime tree-sitter 0.25.10) | hand-written action scan (`template_actions` rules, no tree) |
|---|---|---|
| files with an error (of 1,389) | 2 (0.1%): both invalid Go templates (below) | 0 |
| parse time, whole corpus (9.7 MB), best of 5 | 0.334 s, 29.1 MB/s | 0.011 s, 854 MB/s |
| damaged files (middle `}}` removed): reported | 99.1% (1,374 of 1,387) | 39.5% (548 of 1,387) |
| damaged files: share of the text in error nodes, median | 0.3% | none (no tree) |
| probes correct (16: 13 valid, 3 invalid) | 15 (accepts a stray `{{ end }}`) | 16 |
| stripped binary (empty program 332 KB) | 529 KB (+197 KB) | 398 KB (+65 KB) |
| dependencies | grammar not on crates.io; the repository's crate binds `tree_sitter_go_template`, the parser exports `tree_sitter_gotmpl` (the crate's `language()` does not link); parser built from the repository by `build.rs` | none |
| licence, last push (GitHub) | MIT; 143 stars; pushed 2026-10-02 | own code |

The two files the grammar rejects are invalid Go templates, which `helm template` confirms: `kurtwuckertjr__wazuh-helm/templates/wazuh-master-sts.yaml` (a multi-character single-quoted string: "parse error ... 'wazuh/wazuh:3.10.2_'") and `mstoebich__helm-cluster-oauth/templates/oauth.ldap.yaml` (`.Values.ldap-name`, and `mail` as a function that is not defined). The hand-written scan accepts both.

Probes (`scripts/evidence_helm_parsers/probes`): the grammar reads every valid probe (define, `include | nindent`, two-variable `range`, `with`/`else`, `else if`, comments with trims, a `}}` in a string, `required | default | quote`, `block`, variables, `tpl`, `toYaml | indent`) and rejects the unclosed action and the unclosed block; it accepts a stray `{{ end }}`. The hand scan rejects all three invalid probes.

YAML host (`scripts/evidence_helm_host.py`, the existing Sudsy YAML parser through `sudsy format` on each file, the debug build of the slot: `cargo build -p cli`): raw files as written parse in 67 of 1,389 (4.8%); with the actions masked (own-line actions become blank lines, other actions become `x` of the same length) 1,302 of 1,389 (93.7%). So the host YAML can be read as embedded regions of the action layer.

Reading for the choice (the user decides):
- tree-sitter-go-template: detects 99% of the damaged files and both invalid corpus templates, and gives a tree of the expressions (the pipeline and the `indent`/`nindent` spacing). Costs: a parser built from a repository (not a crate), a binding that does not link, the runtime of the 2026 CLI (tree-sitter 0.25 or later; the core runtime must be checked), and the stray `{{ end }}` check (the shared `template_actions` block count does it). The integration is a grammar file and a `lang.yaml` (the `core-treesitter` engine, as for Rust).
- hand-written over `template_actions`: no dependency, the smallest binary and the fastest, and the block rules are the shared ones. It accepts invalid expressions and detects 39.5% of the damaged files, so it would need its own expression check before the formatter may refuse a file.
- `dialects/helm` of the same repository (scope `source.helm`, a Helm dialect of the grammar) was not measured: its `parser.c` is not a crate (open item).

Recommendation: tree-sitter-go-template (`gotmpl`) for the action layer, with the shared `template_actions` block count for the structure, and the existing YAML grammar for the host as embedded regions. Nothing is built; the choice is the user's.

## Not verified
- Parser: `dialects/helm` (not measured); the core runtime version of this repository against the grammar's ABI 15 (not checked); the speed of the YAML host (the existing YAML leg; not measured here); the trivia (comments and whitespace) of the tree; the grammar's behaviour on the Helm-specific functions beyond the probes.
- Format: stability, harm and meaning checks (no formatter exists); the choices beyond the table (pipelines, `include` arguments, `if` conditions, blank lines inside an action's YAML).
- Lint: no checklist; `helm lint` messages not judged for precision.
- Corpus: batch 2 files are single blobs, so their `helm lint` and their rendered output are not measured; the batch 2 search is the top 1,000 of a relevance order, so its projects are not a sample of Helm charts.
- Usage totals of code search are approximate API counts; star counts and dates are as of 2026-11-26.
