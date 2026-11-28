# Plan: interactive `sudsy init`

Today `sudsy init` writes an empty `extends` list unless the user knows the preset names; migration (`--from`), several presets and the agent block are flags to look up. In a terminal `sudsy init` asks instead.

## Flow
Every question has a default taken from the repository, so pressing Enter through all of them gives a sensible config. Ctrl-C or Esc at any point writes nothing.

1. **Languages.** Scan the working directory (the CLI's own file walk: ignore files respected, capped in time and files) and list the languages found with file counts, all selected. A deselected language is left out of the config the way the config already allows it; if it offers no way, the question only decides which presets are asked, and says so.
2. **Style, per selected language** (one screen, a choice each, not a dialog per language when one answer fits several): keep the tool the repository uses (its config was found), a preset by name with one line of description, or Sudsy's defaults. Presets come from the schemas; only the ones of that language are offered, recommended first.
3. **Existing tool configs** found by the migrators' own file lists (Prettier, ESLint, Biome, stylelint, yamllint, StyleCop, CSharpier, HTML tools): offer to migrate each. Several at once need a merge of the migrators' outputs: shared code in `out.rs`; where two set the same key differently, ask, never drop one silently. Sources that cannot be read (JavaScript configs) are named with what to do.
4. **Lint level:** likely bugs only (the default), or the preset's full rule set for the languages with a preset.
5. **AI agents:** add the block to the agent instructions: `AGENTS.md`, `CLAUDE.md`, or both, with the detected one preselected; default no when neither file exists. Either choice also merges the Claude Code hooks into `.claude/settings.json` (config/cli.md, agent hooks).
6. **Editor and CI:** recommend the VS Code extension in `.vscode/extensions.json` and format on save for the selected languages; write a GitHub Actions job. Each off by default, shown only when the folder exists or the repository is on GitHub; an existing file is merged or left alone, never overwritten.
7. **Summary:** the files to be written and the config text, then confirm. Afterwards the next command (`sudsy check`) and, for a migration, the report of what did not map.

## Rules
- **Never blocks a script.** Questions only when stdin and stderr are terminals and no CI variable is set. Without a terminal, `init` does what it does today. Every question has a flag (`--preset`, `--from` repeatable, `--agents`, new ones for the rest), `--yes` takes all defaults, and any given flag answers its question. Existing flags keep their meaning.
- **One model.** Questions and flags fill the same answers struct; one function turns answers into files. The questions hold no logic of their own, so tests run on the struct, and the prompts are tested with scripted input.
- Data decides: languages, presets, descriptions and tool config files come from the registries; no language names in the init code beyond the migrators.
- Prompt library: measure `dialoguer`, `inquire` and `cliclack` (binary size, dependencies, Windows, maintenance, licence) against a small own prompt module, pick the smallest that gives multi-select, select and confirm with a plain fallback for dumb terminals; startup of every other command unchanged (bench).
- Accessible: works without colour, with `NO_COLOR`, in a narrow terminal and on Windows.

## Library measured
Release, stripped, one prompt each of multi-select, select and confirm, over a hello-world of 332,520 bytes: `dialoguer` 0.12 without default features +65 KB and 5 crates (`console`, `libc`, `shell-words`, `unicode-width`); `inquire` +266 KB, 37 crates; `cliclack` +197 KB, 58 crates. An own module needs `console` for raw mode anyway (`unsafe` is forbidden). `dialoguer` is used; `ctrlc` (+0.1 KB, 6 crates with `nix`) restores the cursor on Ctrl-C.

## Done when
A new user in a mixed repository gets a working `sudsy.yaml` without reading a flag list; `sudsy check` runs on the result without config errors for every combination the tests cover; docs (`docs/configuration.md`, `docs/ai/config/cli.md`, README if it names init, the site's Getting started with a terminal recording as text) are updated.
