# Plan: landing page of the site

What goes on the landing page, from a look at what comparable sites show (read on 2026-11-25; RuboCop's tabbed terminal sessions and install tabs and dprint's card grid and sections were looked at in a browser). Structure only: no copied text, art or styling. How it is built: [development/site.md](../development/site.md#landing-page).

## What other sites show
| Site | Above the fold | Below |
|---|---|---|
| Biome | one-line claim, install command with a tab per package manager, two buttons | formatter before/after, with the measured compatibility with the tool it replaces |
| RuboCop | claim, install command, browser demo | real terminal sessions (check, diff, fix), switchable |
| dprint | claim, small terminal | numbered reasons, install tabs, what it supports, by plugin |
| Oxc | claim, two buttons | one card per tool with numbers, a benchmark chart, sponsors |
| Ruff | claim, benchmark chart, list of reasons | replaced tools, users, quotes |
| ESLint | claim, quick install | find, fix, configure; news; sponsors |
| Prettier | claim, try it, install | what and why, supported languages, editors, users |
| golangci-lint | claim, one button | six short reason cards |

Common to the useful ones: the claim, a command to copy, real output, numbers against the tools people use now, the supported languages or tools, editors. Quotes, users, sponsors, news and an online playground need things Sudsy does not have; they are left out.

## The page, top to bottom
1. **Hero.** Tagline (unchanged: "Fully featured polyglot formatter and linter for 20+ languages. Extremely configurable, plugin-extendable."), install command with tabs (npm, PyPI, Homebrew, Scoop, binary; the same source as the Getting started page), copy button, "Get started" and "Rules".
2. **Real output.** One terminal block with two or three switchable sessions on a small mixed repository: `sudsy check`, `sudsy fix`, `sudsy format` as a diff. The text is produced by running the CLI in the site build or a checked-in generated file with a freshness test, never typed.
3. **One tool for all of them.** The README's before/after of a CI step (a command per tool against `sudsy check`), and the config that replaces them: `sudsy.yaml` with two presets and one rule.
4. **Language grid.** One tile per language of the support table: name, status badge, measured format and lint match with the tool's name, linked to the language page. Generated from `site.json`; the not-ready ones are shown as such.
5. **Speed.** A few bars from the generated speed table, chosen by a rule written in the generator (the primary reference tool of the five languages highest in the support table), slower results of the chosen rows included, with a link to the full table and method.
6. **Reasons**, at most six short cards, each linked: keep your style (presets), hundreds of rules with explanations, safe fixes and the meaning check, your own rules (YAML patterns and Wasm plugins), editor and language server, CI output (GitHub annotations, SARIF).
7. **Last call.** Install command again, links to the docs, rules and GitHub; the early-release note ("v0.0.x, config may change before 1.0").

## Rules
- Every number comes from generated data (`site.json`, the speed table); nothing is typed into the page, and claims that the data does not support are left out.
- Light and dark theme, 360 px without horizontal scrolling, usable without JavaScript (tabs show the first panel), no external fonts or scripts, keyboard reachable tabs, reduced motion respected. Page weight and the largest-contentful-paint time go into `docs/ai/performance/numbers.md`.
- The page uses the components and badge styles of the rule pages; the install tabs are one component used by Getting started too.
- `site/test/smoke.mjs`: each section present, the grid holds every language of the support table, tabs switch, the install commands equal the README's.
