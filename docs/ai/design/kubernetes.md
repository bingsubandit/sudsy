# Design: Kubernetes evidence

Measurements before any Kubernetes product code (method: [../development/no-reference-tool.md](../development/no-reference-tool.md), [../architecture/adding-a-language.md](../architecture/adding-a-language.md)). Kubernetes manifests are YAML: no new parser. Scripts: `scripts/evidence_kubernetes*.py` (facts for `scripts/evidence.py` and `scripts/evidence_lint.py`, rule verdicts, plan per tool, format comparison, schema data, usage counts). kubeconform 0.8.0, kube-linter 0.8.3, kube-score 1.20.0 (aarch64 release binaries, default flags), Prettier 3.10.1, Sudsy release build. Looked up 2026-11-26.

## Corpus
81 projects pinned by commit in `tests/conformance/fetch-kubernetes-corpus.sh`, listed before any result (the Kubernetes organisations' examples and docs, operator and controller repositories with their `config/` and `deploy/`, GitOps and homelab repositories, sample applications, CRD-heavy projects: Argo, Flux, Tekton, Istio, Cilium, Calico, Kyverno, Gateway API, Strimzi, Rook, kubeflow; Helm chart folders `charts/` and `templates/` are left out). Every `*.yml` and `*.yaml`: 35,372 files, 773 MB. Of 82 candidates one is not on GitHub (dropped).

Manifests (`evidence_kubernetes.py detect`): 26,283 files (74%) have a top-level `apiVersion:` of the form `[group/]vN[alpha|beta]N` and a `kind:` starting with a capital letter, and no `{{`; 2,058 files with `{{` (Helm-like) are left out; 4,528 manifests hold several documents. Kinds by objects: Deployment 3,681, Pod 3,522, Kustomization 2,999, Service 2,618, CustomResourceDefinition 2,381, ClusterRole 1,662, ConfigMap 1,168. The corpus for the lint and format legs is the manifests, at most 60 per project (first by path): 3,839 files of 75 projects. Split by project with seed 20261204: 38 deciding, 37 held-out projects.

## Recognition
Nothing in the path says a file is a manifest (no `*.k8s.yaml` convention; names are `deployment.yaml`, `kustomization.yaml`, `base/`, `overlays/`, `config/`, `manifests/`, `deploy/`). Recognition is by content, as kubectl and kubeconform do: a document with `apiVersion` and `kind`. Measured: a first rule (any `apiVersion:` and `kind:` line) matched 30,042 files, 3,759 more than the final rule: Istio release notes (`apiVersion: release-notes/v2`, `kind: feature`, 1,979 files) and similar. The final rule needs an apiVersion of the form `[group/]version` and a capitalised kind. Open: a content rule is not a path kind, so it is one more way to pick a dialect (A1 of [ansible.md](ansible.md) picks by path; GitHub Actions by path); the dialect of a YAML file chosen by the top-level keys of its first document is a second use of that shared mechanism only if another language needs it (Docker Compose `services:`, OpenAPI `openapi:`, Ansible by content, 14% of its corpus).

## Tools (usage evidence, `scripts/evidence_kubernetes_usage.py`)
| Tool | Evidence | Verdict |
|---|---|---|
| kubeconform 0.8.0 (Apache-2.0, Go) | Homebrew 32,103 installs in 30 days, 3,270 stars, pushed 2026-06-13; `kubeconform` in 6,048 workflow files and 207 `.pre-commit-config.yaml`; 2,200 workflows pass `-ignore-missing-schemas` | primary (validity); replaced kubeval (1,020 workflow files, last push 2026-01) |
| kube-linter 0.8.3 (Apache-2.0, Go; StackRox) | Homebrew 1,882, 3,532 stars, pushed 2026-11-25; 512 workflow files, 52 pre-commit, 389 `.kube-linter.yaml|yml` files; VS Code extension `stackrox.kubelinter` | second reference (policy: security and resources); 31 checks on by default of 63 |
| kube-score 1.20.0 (MIT, Go) | Homebrew 52, 3,103 stars, pushed 2026-05-20; 219 workflow files, 13 pre-commit | not a reference (its checks are a subset of kube-linter's in kind, with opinions: replicas, pull policy, user id) |
| kubescape, Polaris, Trivy config, Checkov, Conftest, Kyverno CLI, datree | kubescape 1,049 Homebrew installs, 11,990 stars; Polaris 43; datree archived 2024-04; Checkov 5.6M PyPI downloads a month and Trivy 39,755 stars are IaC scanners of many formats; conftest 13,456 workflow hits are mostly pytest `conftest.py`; kyverno 858 | policy engines and multi-format scanners, not manifest linters; not measured |
| Red Hat YAML extension | 30.2M installs; its language server validates Kubernetes files against the Kubernetes JSON schemas, the same data as kubeconform | the editor view of the same check |
| Kubernetes extension (Microsoft) | 7.7M installs | not a linter |

kubeconform is the tool people run (the Homebrew count is 17 times kube-linter's, workflow files 12 times). Its default rule set is schema validation only, and in practice almost no finding is a real error (see Target); the policy checks that people want from a lint are kube-linter's. Decision for the user below.

## Target
Per-file runs, default flags, no config, offline except kubeconform's schema download (cache): 12,681 findings of 60 check codes in 3,080 of 3,839 files, no tool failure (the tools' own parse failures are 24 findings). kubeconform and kube-linter name an object, not a line: a finding is placed on the `kind:` line of the document of its object (kind and name). Rules read: 4 findings each, over distinct projects, for (`kubeconform` required, other, parse-error, tool failures, `mismatching-selector`, `non-existent-service-account`, `deployment-replicas`, `container-security-context-user-group-id`); the rest by name and message (`*`).

| Class | Rules | Findings | Share | Weighted by project | Held-out findings |
|---|---|---|---|---|---|
| keep | 36 | 8,313 | 65.6% | 70.9% | 4,478 |
| taste (kube-score: replicas, pull policy, user id, storage limits; kube-linter: anti-affinity, job ttl, PDB policy) | 12 | 2,601 | 20.5% | 16.5% | 1,616 |
| env: reads other objects of the project (a Service with no Deployment in the file, a serviceAccount defined elsewhere, a NetworkPolicy or PodDisruptionBudget) | 10 | 1,743 | 13.7% | 12.4% | 969 |
| artifact: the tool's own YAML parser failed | 2 | 24 | 0.2% | 0.2% | 21 |

Per tool (`evidence_kubernetes_plan.py`; the target is keep and taste):
- kubeconform: 2,816 findings in 73 projects, all keep. `schema-missing` 2,719 (96.6%: the kind and apiVersion have no schema in the default folder: CRD kinds, `CustomResourceDefinition` itself 731, `Kustomization` 457, Flux, Argo, Tekton, Cilium, Istio), `required` 80, `parse-error` 11, other 6. Of the 86 schema findings, 76 are partial manifests (strategic-merge patches that kustomize completes; status stubs): kubeconform reports a `spec` without `selector` or `template`. So a real schema error is about 10 findings in 3,839 files (a number for a string, `got boolean, want string`, an `apiVersion` that does not match the kind). `-ignore-missing-schemas` appears in 2,200 workflow files (36% of the kubeconform count); those runs see only the 97 schema and parse findings.
- kube-linter: 4,042 findings in 64 projects, keep 3,585 (21 rules), taste 123 (3), env 324 (2: `dangling-service`, `non-existent-service-account`). Target 3,708 findings of 24 rules.
- kube-score: 5,823 findings, keep 1,912 (11), taste 2,478 (9), env 1,419 (8: `pod-networkpolicy` 779, `service-targets-pod`, PDB, HPA, Ingress targets). Target 4,390 of 20 rules; it overlaps kube-linter (read-only root fs, resources, privileged, latest tag, probes).

Schema data. Kubernetes OpenAPI `api/openapi-spec/swagger.json` (Apache-2.0): 4,381,165 bytes, 838 definitions, 3,325 properties. A condensed key-shape table (definition name, property names with a type tag `s`, `i`, `[@Ref]`, `{s}`, the required list) is 136,041 bytes raw and 18,073 gzipped (`evidence_kubernetes_schemas.py`); enums, patterns, formats and descriptions are left out. The default kubeconform folder `yannh/kubernetes-json-schema` `master-standalone` has 1,481 files (the newest versioned strict folder, `v1.34.12-standalone-strict`: 1,304 files, 56.5 MB; the 46 files the corpus used: 7.3 MB, 0.85 MB gzipped); the repository has no SPDX licence (NOASSERTION), so the table is generated from the OpenAPI spec, not copied. A table of kind and group/version, as kubeconform names its files, decides `schema-missing`: against the findings per object, 2,717 both, 1 table only, 5 kubeconform only, 3,181 neither (the table is the 1,481 file names, about 40 KB of text). Of the 294 corpus GVKs not in the table, the CRD catalog `datreeio/CRDs-catalog` (MIT, 4,407 schemas in 612 groups) has 160 (1,423 of 2,718 objects); the others are CRD, IstioOperator, Calico `Node`, Tekton `TaskRun`, KinD `Cluster`.

How the shared table is held (for the rule step): a key-shape table is a data file of rows `definition: {key: type-tag}` plus required lists, read by the existing `core_rules::shape` walk (path with `*`, allowed and required keys, kind) that GitHub Actions uses as a Rust table; Kubernetes is the second user, so the table moves into data rows and the walk stays shared. 136 KB raw, 18 KB gzipped: no size problem for a build-time table. Values and enums need the full spec (not measured).

## Baseline (measured with the release build; no rule is written)
Lint: Sudsy has no Kubernetes rule: 0% of every target. `sudsy lint` over the 3,839 files: 0.48 s, 35 output lines (generic YAML rules). Tool times were not measured.

Format (byte-equal files, deciding, held out; 38 and 37 projects):
| Pair | Files |
|---|---|
| Sudsy default = Prettier | 1,941 of 3,839 (50.6%), deciding 930, held out 1,011 |
| Sudsy `yaml/prettier-compat` = Prettier | 3,316 (86.4%), deciding 1,667, held out 1,649 |
| original = Prettier | 1,727 (45.0%) |
| original = Sudsy default / prettier-compat | 3,192 (83.1%) / 1,704 (44.4%) |

Second format: 0 files change, both variants. The first differences of the remaining 14%: long quoted scalars of CRD descriptions that Prettier folds onto a following line (the largest group: CRD `description:` scalars, about 60 of the files in the 12 most common first differences), quoted multi-line strings, comment indent inside sequences, a leading blank line. Format needs no Kubernetes mechanism: the preset `yaml/prettier-compat` is the whole answer (the Sudsy default and the preset differ by 36 points of files, mostly the sequence indent); the remainder is the YAML leg's scalar folding, not Kubernetes.

## Plan to 85% (lint)
Rules in order of findings, cumulative share of the target (raw / weighted by project / held out):
- kubeconform: 1 `schema-missing` 96.6 / 95.5 / 97.0; 2 `required` 99.4 / 99.2 / 99.1. One rule reaches 85%.
- kube-linter: 1 no-read-only-root-fs 20.3 / 18.7 / 22.2; 2 run-as-non-root 40.1 / 36.0 / 43.4; 3 unset-memory-requirements 58.8 / 53.9 / 62.5; 4 unset-cpu-requirements 75.9 / 69.6 / 79.1; 5 latest-tag 85.7 / 82.2 / 89.4; 6 no-anti-affinity 87.6 / 84.4 / 91.4; 7 mismatching-selector 89.5 / 87.1 / 92.5. 85% raw and held out with 5 rules (weighted: 7). The first four are one family: a container of a pod spec lacks a key (securityContext readOnlyRootFilesystem, runAsNonRoot, resources limits and requests).
- Both pooled (6,524 findings): schema-missing and the five kube-linter rules reach 90.4% raw (84.8% before `latest-tag`).
- kube-score: 7 rules for 85% (raw, weighted, held out), the same families as kube-linter with other thresholds.

Parts (shared crates; the language holds tables and options):
- K1: `k8s-unknown-schema` as a data table of kind and group/version and one shared rule "a document's pair is not in the list" (a table of names, `word-table` format; second use: any schema-by-kind format).
- K2: pod-spec context: the containers of a document by a table of kind to path (`Deployment`/`StatefulSet`/`DaemonSet`/`Job`/`ReplicaSet`: `spec.template.spec`, `CronJob`: `spec.jobTemplate.spec.template.spec`, `Pod`: `spec`), `containers` and `initContainers`; the family of key-missing rules over it (pattern rules with `has` / `inside` over the kind table). Shares the task-context shape of A2 in ansible.md (a table says where the unit is).
- K3: kind detection by content (the recognition above) and a preset `yaml/kubernetes` (prettier-compat plus the rule set); the dialect has no new grammar.
- K4 (deferred): key-shape check from the OpenAPI table (required and type errors; 3.4% of kubeconform, 0.8% pooled; 76 of its 86 findings are patches that are not errors).
- Not planned: the env rules (Service with no Deployment, NetworkPolicy and PDB presence: they read other files of the project); CRD schemas (a downloaded catalog); kustomize overlays.

## Decisions for the user
1. Primary: kubeconform by usage; but its default set is the one-rule `schema-missing`, a result of its default flags that 36% of the workflow count switches off. Recommendation: kubeconform as the validity reference and kube-linter as the policy reference, the bar 85% of both pooled (6 rules, parts K1 to K3). Alternative: kube-linter alone (5 rules, 85.7%).
2. Whether `schema-missing` belongs in the default rule set of a linter (it is noise for CRD users): a rule that is off in the Sudsy preset and on in a `kubeconform` preset keeps the measure and the zero-config result clean.

## Built (lint)
Pattern rules in `crates/lang-yaml/rules/kubernetes-*.yaml` (off by default, on in preset `yaml/kubernetes`): `kubernetes-schema-missing` (option `known`, 180 `apiVersion/Kind` pairs from the OpenAPI spec filtered by the kubeconform folder, `scripts/kubernetes_kinds.py`), `kubernetes-no-read-only-root-fs`, `kubernetes-run-as-non-root` (the pod's or the container's `runAsNonRoot: true` or `runAsUser` above 0), `kubernetes-unset-memory-requirements` (`limits.memory`), `kubernetes-unset-cpu-requirements` (`requests.cpu`), `kubernetes-latest-tag`, `kubernetes-host-namespace` (`hostNetwork`, `hostPID`, `hostIPC`), `kubernetes-privileged-container`, `kubernetes-privilege-escalation`, `kubernetes-drop-net-raw-capability` (a `capabilities` block without `NET_RAW` or `all` in `drop`), `kubernetes-job-ttl-seconds-after-finished`, `kubernetes-no-extensions-v1beta` (`extensions/v1beta1` and `apiextensions.k8s.io/v1beta1`, any kind). Shared pieces: pattern atoms `top-level` (recognition by content, captures at the value) and `top-level-in-option`; the plain option words of a default-off rule may hold `/`. Reports are on the `kind:` line. Coverage of the target (keep + taste, same file and line, `scripts/evidence_kubernetes_baseline.py`): pooled 89.4% raw (5,834 of 6,524), deciding 86.8%, held out 91.8%; kubeconform 96.1% (held out 96.7%), kube-linter 89.9% (held out 91.3%). Each built rule covers 99% or more of its findings except `latest-tag` (88.6%, deciding 79.1%). Open: the key-shape check (K4), `no-anti-affinity` and `mismatching-selector` (kube-linter, 1.9 points each), the env rules.

## Performance
`sudsy lint` 0.48 s and format 0.75 s (prettier-compat 0.89 s) over the 3,839 files (about 36 MB of manifests). The three Go tools take about a minute per tool and run per file here (not timed). Kind-table and container-context rules add a lookup per document; not measured, no code exists.

## Cost
Parser, format and lint are built (`languages.yaml` no longer lists the language). Measured by `scripts/parity.sh`: format Prettier 86.2% (3,299 of 3,826 files, second format 0), lint kubeconform 96.1% and kube-linter 89.9% (each tool's own run, one process per file). The corpus, the verdict classes and the plan above are the evidence; the numbers of the parity runs are in the README table.

## Not verified
Rule behaviour beyond the findings read; whether kubeconform's default Kubernetes version (`master`) is the same schema set the table was built from over time; a run of kube-linter and kube-score per directory (cross-object checks, 13.7% of findings, were per file); Kustomize patch recognition (a `kustomization.yaml` run); the licence of the generated table beyond Apache-2.0 for the OpenAPI source; CRD schemas for the 134 corpus kinds not in any catalog; tool wall times; Windows paths.
