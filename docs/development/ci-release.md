# CI and release

## CI (`.github/workflows/ci.yml`, reusable via `workflow_call`)
Runs on pushes to main, pull requests, dispatch and from the release workflow. A run for a push to main is never cancelled; other refs cancel their previous run. `scripts/ci-changes.sh` (job `changes`) skips the gates for changes that only touch root prose files, the license, bench history or `site/` (generated docs live in `docs/`, so those always run) and runs `package` only when `editors/vscode/`, `workers/csharp/`, `crates/lang-csharp/src/` or the workflows change (always on dispatch and tags). `scripts/gate.sh` stays the local pre-merge check. The Rust version is pinned in `rust-toolchain.toml` (CI and local builds both use it); a new stable is adopted in its own PR once CI passes on it.

`main` has a ruleset requiring the `CI ok` job (green when every job that ran passed); repository admins bypass it, so agents push directly. Auto-merge and branch deletion on merge are on.
| Job | Does |
|---|---|
| `changes` | what the change touches (above) |
| `dco` | pull requests only: every commit signed off by its author (`scripts/check-dco.sh`) |
| `rust` | Linux: fmt check, clippy `-D warnings`, `cargo test --workspace`, `cargo xtask codegen --check`, sample plugin fmt + tests (installs `wasm32-wasip2`) |
| `windows` | `cargo test --workspace` only (it builds and runs the worker, so there is no Windows dotnet job) |
| `bench` | Dependabot pull requests only: calls `finish.yml` (bench job only) with `--all --no-record` against the PR base |
| `dotnet` | Linux: `dotnet build` / `dotnet test workers/csharp` |
| `vscode` | `npm ci`, typecheck, lint, test, build (Linux) |
| `packages` | dry run of the package-manager packages (below) from the latest release's assets (`scripts/package-dry-run.sh`); only when packaging inputs changed |
| `package` | release `sudsy`, published worker, staged server, `.vsix` artifact for `linux-x64`, `win32-x64` and `darwin-arm64` (3-day retention), after a smoke test that the staged binary formats C# without `SUDSY_CSHARP_WORKER` |
| `ci` | `CI ok`: the required check |

`.github/workflows/finish.yml`: the check of a finished change (inputs `ref`, `base`, `args`, `gate`, `gate_args`, `token`, `head`, `dirty`; `workflow_dispatch` and `workflow_call`). Jobs, in parallel: `gate-linux` (`scripts/gate.sh` with `gate_args`, after the codegen check; Rust, .NET, Node, nextest) and `bench` (tier 1 of `scripts/bench.sh` against `base`, both sides built on ubuntu-latest with valgrind; its rust-cache also holds `target/bench/{cand,base}`). The gate jobs run when `gate` is `1`. Every job has a 60 minute timeout; the caches are saved by every run that is not a pull request (the key follows `Cargo.lock`). `scripts/finish.sh` dispatches it with `--ref main` for a snapshot commit it force-pushes to `bench/<slot>` (these branches match no trigger of `ci.yml`, so the full CI does not run; each is deleted when its finish ends) and finds the run by its title `finish <token>`; `ci.yml` calls it with the gate off for Dependabot PRs. The artifact `bench-result` holds `bench-output.txt` and `bench-line.jsonl`. GitHub dispatches only workflows whose file is on the default branch.

`.github/workflows/site.yml` builds the documentation site and deploys it to Pages from main, see [site.md](site.md).

Dependabot (`.github/dependabot.yml`: cargo, npm, nuget monthly, actions weekly, one grouped PR each) pull requests run this CI plus `bench`; `dependabot-automerge.yml` enables rebase auto-merge for patch and minor updates, majors wait for a human.

## Release (`.github/workflows/release.yml`, on a `vX.Y.Z` tag)
1. `scripts/check-version.sh`: one version for the Cargo workspace, `editors/vscode/package.json` + lock, the tag, and a `CHANGELOG.md` section. The sample plugin stays 0.0.0.
2. All of `ci.yml`.
3. One build job per platform on a native runner (linux, win32, darwin; x64 and arm64): packs (`scripts/pack.sh`), the pack-mode CLI embedding their hashes, the CLI archive and the offline `sudsy-full` archive with `packs/` (`scripts/package-cli.sh`), the pack files themselves as assets, and the platform `.vsix`, after the same smoke test ([language packs](../architecture/language-packs.md)).
4. A job drafts a GitHub release with archives and `SHA256SUMS`, notes from the changelog section. `-rc.N` suffixes make a pre-release.

Publishing is manual: the workflow publishes nothing by itself. The drafted release must be published before language packs can download: a draft's assets return 404. The `marketplace` job runs only from a hand-started run on a tag with its input ticked, using the `VSCE_PAT` secret. The package managers below are published the same way, by `publish.yml`. Nothing goes to crates.io (every crate is `publish = false`; the SDK is used by path until the plugin API is stable) or NuGet (the worker ships inside the packages).

## Package managers (`.github/workflows/publish.yml`, `packaging/`, `scripts/package-channels.py`)
Every channel repackages the pack-mode release archives (executable plus `csharp-worker/`; language packs download on `sudsy languages add`), so nothing is built again. `scripts/package-channels.py <channel|all> <version> <assets> <out> [tag]` renders a channel from the assets of a published release (fetched with `gh release download` when the assets dir is empty); templates are in `packaging/` (`@VERSION@`, `@URL <platform>@`, `@SHA <platform>@`, hashes from `SHA256SUMS`). `scripts/package-dry-run.sh [tag] [dir]` renders everything into a temp dir, checks it (JSON, wheels, the npm shim running this machine's binary) and publishes nothing; CI runs it on the latest release.

Publishing is by hand: publish the draft release, then start `publish.yml` on its tag (Run workflow, pick the tag) with one input ticked per channel; or start it on `main` with the `tag` input set, which publishes that release with the current packaging scripts (for a release whose own scripts are out of date). A tag push runs nothing there. A job whose secret is missing is skipped with a warning.

| Channel | Package | Needs (one time) | Install |
|---|---|---|---|
| npm | `sudsy` (bin shim) with `@sudsylint/cli-<os>-<arch>` as optionalDependencies, six platforms, `npm publish --provenance` | create the npm organisation `sudsylint` (the scope), secret `NPM_TOKEN` (automation token that may publish to it) | `npm i -D sudsy` or `npx sudsy` |
| PyPI | `sudsy`: one `py3-none-<platform>` wheel per platform, the binary and worker inside the package, a console script starts it | trusted publisher on pypi.org for project `sudsy`: owner `bingsubandit`, repo `sudsy`, workflow `publish.yml`, no environment (a pending publisher creates the project on first upload); no secret | `pip install sudsy`, `uvx sudsy` |
| Homebrew | `Formula/sudsy.rb` pushed to the tap | create repo `bingsubandit/homebrew-sudsy` (with a first commit), secret `HOMEBREW_TAP_TOKEN` (contents: write on it) | `brew install bingsubandit/sudsy/sudsy` |
| Scoop | `bucket/sudsy.json` (with `autoupdate`) pushed to the bucket | create repo `bingsubandit/scoop-sudsy`, secret `SCOOP_BUCKET_TOKEN` (contents: write on it) | `scoop bucket add sudsy https://github.com/bingsubandit/scoop-sudsy` then `scoop install sudsy` |
| winget | manifests `Coox.Sudsy` (zip with a nested portable `sudsy.exe`, x64 and arm64) pushed to a fork of `microsoft/winget-pkgs`, pull request opened | secret `WINGET_TOKEN` (classic PAT, scope `public_repo`; the job forks `microsoft/winget-pkgs` on first use); the first submission passes Microsoft's validation and review | `winget install Coox.Sudsy` |
| cargo-binstall | `[package.metadata.binstall]` in `crates/cli/Cargo.toml` (archive names per target); no crates.io publishing | nothing | `cargo binstall --git https://github.com/bingsubandit/sudsy sudsy` (installs the executable only; C# needs the archive's `csharp-worker/`) |

Platform names are the release's (`linux|darwin|win32` x `x64|arm64`). Wheels are tagged `manylinux_2_39` (the glibc of the build runners), `macosx_10_12_x86_64`, `macosx_11_0_arm64`, `win_amd64`, `win_arm64`. A `-rc.N` tag publishes to the `next` npm tag. The Homebrew formula wraps the binary with `SUDSY_CSHARP_WORKER` set, since Homebrew links `bin/sudsy` away from the worker.

Supply chain: third-party actions are pinned by commit SHA (version in a comment; Dependabot bumps them weekly), workflows default to `contents: read` (only `release` writes), the tag name reaches scripts through `env`, and the `marketplace` job runs `vsce` from the `package-lock.json` install, not an unpinned download.

## Identity and contributions
- Marketplace publisher `Coox` (extension id `Coox.sudsy`); the repo stays under the owner's GitHub account.
- MIT, copyright Maia Coox; the `.vsix` ships the root `LICENSE` (copied at build).
- Logo: the Fluent UI Emoji "Soap" (MIT, Microsoft) in `assets/logo/` with its license; the `.vsix` icon (`icon.png`, copied at build), the site header logo and favicon, and the README use it. Attribution: `THIRD-PARTY-NOTICES.md`.
- Contributions need a DCO sign-off (`git commit -s`, see `CONTRIBUTING.md`); the `dco` CI job runs `scripts/check-dco.sh` on pull requests. No CLA.

## Still open
- Pack builds and `languages add --from` are proven locally on Windows x64 and Linux x64; macOS and arm64 builds are untested on real hardware until the first release run.
- The first release is 0.0.1; `v0.1.0-rc.1` is an older tag that was never published.
