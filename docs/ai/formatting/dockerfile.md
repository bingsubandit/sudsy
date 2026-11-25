# Dockerfile (`lang-dockerfile`)

## Parser
tree-sitter-containerfile 0.9.2 (MIT, WharfLab's maintained fork of tree-sitter-dockerfile, forked again in `grammars/tree-sitter-containerfile`, see its FORK.md; follows BuildKit: heredocs, `--mount`, `# escape=` and `# syntax=` directives) linked in natively and converted by `core-treesitter` ([../architecture/adding-a-language.md](../architecture/adding-a-language.md)); the node mapping is the `mapping` of `crates/lang-dockerfile/lang.yaml`, the tree shape `grammars/dockerfile.grammar`. A `Document` holds `Instruction`s; an `Instruction` is its `Keyword` and its arguments as flat tokens, plus the instruction `ONBUILD` or `HEALTHCHECK` holds. Options (`--chown=...`), paths, images, `KEY=value` pairs and quoted strings are one `Word` token each, the command of `RUN`/`CMD`/`ENTRYPOINT`/`HEALTHCHECK` one `ShellCommand` token (the shell of `RUN` is parsed beside the tree, see Rules), a heredoc body one `Heredoc` token. `STOPSIGNAL`, `MAINTAINER` and `CROSS_BUILD` hide their tokens in the grammar, so each is one `Raw` token. The grammar tracks the escape character itself: after `# escape=` + backtick, the line continuation token starts with a backtick.

Files: `Dockerfile`, `Containerfile`, `Dockerfile.*`, `Containerfile.*` (`Dockerfile.dev`) and `*.dockerfile`. A `file_names` entry ending in `*` matches by prefix.

A `#` is a comment at the start of a line or after a blank, not after other text (`ADD ${A}#${B} /`), and `${V#pattern}` is an expansion (`#`, `##`, `%`, `%%`); both are changes of the vendored grammar ([FORK.md](../../../grammars/tree-sitter-containerfile/FORK.md)). No file of the pinned corpus has an `ERROR` node in its tree.

## Formatting
- The instruction name and `AS` are upper case (`from x as y` is `FROM x AS y`).
- One space between arguments wherever the source has blanks; none where it has none (`USER a:b`, `--chown=1:1`). Options, paths, images, pairs, quoted strings and shell text are kept as written, including blanks inside them (`ENV KEY  some value` keeps the value's blanks).
- JSON-form arguments (`CMD`, `ENTRYPOINT`, `RUN`, `COPY`, `ADD`, `VOLUME`, `SHELL`, `HEALTHCHECK CMD`): `["a", "b"]`, no blanks inside the brackets, one after each comma.
- A line continuation ends its line with one space before it, and what follows is indented one level (`indent_size`, default 4; `indent_style`); this also re-indents the continued lines of shell text and of multi-line pairs. The continued lines keep their indentation relative to the least indented one (a tab counts four columns): that line gets the one level, a deeper line stays that much deeper. Lines inside a quote are kept exactly (the blanks are content). Comment lines inside a continuation stay, indented the same. The escape character is the one the line uses, so `# escape=` is honoured.
- Heredoc bodies and their delimiter line are kept as written (trailing blanks included).
- Comments and parser directives stay where they are; blank lines between instructions are kept up to `max_blank_lines`.
- Not changed: the shell code itself (`&&` chains, `;`), legacy `ENV key value`, `MAINTAINER`. The generic safety check plus `DockerfileFormatter::value` allow only case and blanks to change: every token keeps its kind and text with blanks removed (keywords compared case-insensitively).

dockerfmt (reteps/dockerfmt) is the primary tool of the format leg (`scripts/parity.sh format dockerfmt`, the 563 files of the corpus in two halves by project, `deciding` and `held-out`; [../design/dockerfile.md](../design/dockerfile.md) holds the evidence for the choice). It lays out the shell code of `RUN` with mvdan/sh and rewrites legacy `ENV`, JSON `COPY` and `MAINTAINER`; the open differences are the `gap` names of `tests/parity/dockerfmt/differences.yaml` and `docs/ai/parity/queue.md`. `MAINTAINER` to `LABEL` is not done: it changes metadata.

## Options
<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `indent_size` | int 1-16 | `4` | Columns per indentation level (also the width of a tab). |
| `indent_style` | `space`, `tab` | `space` | Indent with spaces or tabs. |
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
| `max_blank_lines` | int 0-10 | `1` | Largest number of blank lines kept between instructions and comments. |
<!-- /generated -->

## Presets
`dockerfile/hadolint`: turns on the rules that have a hadolint equivalent ([../parity/hadolint.yaml](../parity/hadolint.yaml)) at hadolint's severities (error stays error; warning, info and style become `warn`).

Checked against hadolint by `scripts/parity.sh hadolint` ([../development/parity.md](../development/parity.md#hadolint)).

## Rules
Shared: suppression comments (`# sudsy-disable-next-line <rule>`), `no-empty-comment`, `comment-space`, `max-line-length`. The grammar has no `#mapping` or `#sequence` nodes, so the key rules do not apply. The language rules are listed in [../rules/builtin.md](../rules/builtin.md); the equivalents of hadolint's DL rules are in the catalog.
- Instruction shape (`rules/instructions.yaml`): pattern rules with a regex over the instruction text (WORKDIR, FROM, EXPOSE, CMD, COPY, ADD, USER, ENV).
- Commands of `RUN` (`rules/*.yaml` naming the `run-command` family, `src/rules/run_command.rs` on `core_rules::commands`): the commands come from the Shell tree of the `RUN` (its `Command` nodes: the first word is the program, quotes are dropped; the script of `RUN <<EOT` counts, and so does the JSON form, which has no shell), and a rule selects a program and subcommand, then needs a flag, a pinned package, a later clean command, or just reports the call. A command inside `$(...)`, backticks or `"$(...)"` is a `Command` node of the Shell tree and is read like the others, and a script that does not parse as shell is not checked. A `--mount` of type cache or tmpfs on a directory exempts the cleanup rules for it.
- Shell in `RUN`, `CMD` and `ENTRYPOINT` (the `embed` rows of `grammars/dockerfile.grammar`; also under `ONBUILD` and `HEALTHCHECK`): the shell-form command and the body of a `RUN` heredoc are Shell code ([../architecture/adding-a-language.md](../architecture/adding-a-language.md#embedded-languages)); the exec (JSON) form is not. `SHELL` is not consulted: the code is always parsed as POSIX shell. The comment lines inside a continued command, which Docker drops, are the Dockerfile's part of it, and so is each backtick continuation (with its line break) after `# escape=` + backtick. Only `RUN` feeds the `run-command` family. The shell rules (`cd-without-exit`, the shellcheck ones, the `shell:` section of the config) run on it, and a `# sudsy-disable-next-line` comment of the Dockerfile silences them. `RUN` text is formatted as written (above); only the instruction's line breaks are re-indented.
- Stages (`src/rules/stages.rs`, `rules/once-per-stage.yaml`, `rules/duplicate-stage-name.yaml` on the shared `unique-names` family): untagged base image, last `USER` root, stage names, `COPY --from`, the first instruction, a second `CMD`, `ENTRYPOINT` or `HEALTHCHECK` in a stage.
- On by default: the rules for an invalid or broken Dockerfile (stages, `COPY --from`, ports, `-y` of package managers, `MAINTAINER`, repeated `CMD`, secrets in `ENV`, `USER root` last). Off by default: hadolint's best practices (pinned versions, tags, cache cleaning, `apt`, `sudo`, `WORKDIR`, `ADD`) and its informational checks (`apt-get-no-recommends`, `apk-no-cache`, `no-interactive-commands`, `wget-progress`, `numeric-user-id`); the preset `dockerfile/hadolint` turns them all on.
- Known gap: a `#` inside an `ADD` source (`ADD ${REPO}#${REF} /`) makes the parser take the following lines into the instruction.

## Conformance
`tests/conformance/fetch-dockerfile-evidence-corpus.sh` adds 20 more projects (563 files with this one; the stability group reads both). `tests/conformance/fetch-dockerfile-corpus.sh` fetches the Dockerfiles of dotnet/dotnet-docker (samples and the 10.0 runtime, aspnet and sdk images), docker-library (golang, python, nginx, postgres), moby and buildx at pinned commits; `cargo test --profile corpus -p lang-dockerfile --test integration -- --ignored --nocapture conformance` checks that each file parses (but for the gap above), round-trips, keeps its instructions and arguments (the safety check, with comments compared as well) and formats idempotently.

Where Sudsy differs from hadolint by design:

<!-- generated:differences:hadolint -->

- `rm-fr`: `rm -fr /var/lib/apt/lists` counts as the cleanup (hadolint accepts only the flag order `-rf`)
- `windows-run-busybox`: the base image is `mcr.microsoft.com/windows/servercore` and the RUN line is `RUN powershell ...`: hadolint parses it as sh
- `windows-run-escape`: the file has `# escape=` followed by a backtick, a Windows base image and no `SHELL`: its RUN lines run under cmd.exe, hadolint parses them as sh
- `windows-run-powershell`: the file has a `SHELL` line that starts powershell on a Windows base image: its RUN lines run under PowerShell, hadolint parses them as sh
<!-- /generated -->
