// Agent-facing copies of the docs: a Markdown twin of every page (`/<slug>.md`, `/index.md` for the
// landing page), `llms.txt` (an index) and `llms-full.txt` (the guide pages in one file), and `rules.json`.
// Built from the same pages and data as the site: `generate.mjs` calls `twin()` for each generated page,
// `handwritten()` reads the pages in src/content/docs, and `write()` flushes everything into public/.
import { copyFileSync, mkdirSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { dirname } from 'node:path';
import { base, site, rulesPath, siteData } from '../config.mjs';
import { installMarkdown } from './install.mjs';

const origin = site + base;
const twins = new Map(); // slug -> { title, description, text }

// Page Markdown for a reader without a browser: no layout HTML, no editor meta, absolute links to twins.
// A progress bar reads as its label ("Format: 95% of files identical to Prettier"); in a table cell it stays in the line.
const BAR = /<span class="bar [^"]*">.*?aria-label="([^"]*)".*?<span class="bar-num">[^<]*<\/span><\/span>/g;
const bars = (body) => body.split('\n').map((line) => line.replace(BAR, (_, label) => (line.startsWith('|') ? label : `\n- ${label}`).replace(/&quot;/g, '"'))).join('\n');

function clean(body) {
  return bars(body)
    .replace(/<div class="lang-head"><a href="([^"]*)">([^<]*)<\/a><span class="badge[^>]*>([^<]*)<\/span><\/div>/g, '\n\n**[$2]($1)**: $3')
    .replace(/<div class="leg"><span class="leg-title">([^<]*)<\/span><span class="leg-empty">([^<]*)<\/span>/g, '\n- $1: $2')
    .replace(/^<div class="page-head">(.*)<\/div>$/m, (_, badges) => badges.replace(/<\/span>/g, ' | ').replace(/<[^>]*>/g, '').replace(/ \| $/, ''))
    .replace(/<span class="badge[^>]*>(?:<a[^>]*>)?([^<]*)(?:<\/a>)?<\/span>/g, '$1')
    .replace(/<\/?div[^>]*>/g, '')
    .replace(/ squiggles="[^"]*" tip="[^"]*"/g, '')
    .replace(/<[a-z][^>]*>[\s\S]*?<\/[a-z]+>|<[a-z][^>]*\/>/g, '')
    .replace(/\]\(\/sudsy\/([^)#?]*?)\/?(#[^)]*)?\)/g, (m, path, hash) => (path ? `](${origin}/${path}.md${hash ?? ''})` : `](${origin}/index.md)`))
    .replace(/\]\(\/sudsy\//g, `](${origin}/`)
    .replace(/\n{3,}/g, '\n\n')
    .trim();
}

export function twin(slug, front, body) {
  twins.set(slug, { title: front.title, description: front.description ?? '', text: clean(body) });
}

// The pages of src/content/docs outside generated/: front matter, then Markdown; the rules index has a component.
export function handwritten(dir) {
  // Pages that are a component or a hero in the browser.
  const extra = {
    index: 'Start with [getting started](/sudsy/getting-started/), the [command line](/sudsy/cli/) and the [languages](/sudsy/languages/).',
    rules: 'One page per rule at `rules/<id>.md`; every rule as data in [rules.json](/sudsy/rules.json). Filter by language with the [language pages](/sudsy/languages/); search an id of another tool (`SA1005`, `DL3008`) at `rules/<id>`.',
  };
  const slugs = { 'getting-started.mdx': 'getting-started', 'cli.md': 'cli', 'index.mdx': 'index', 'rules/index.mdx': 'rules' };
  for (const [file, slug] of Object.entries(slugs)) {
    const text = readFileSync(dir + file, 'utf8');
    const [, front, body] = text.match(/^---\n([\s\S]*?)\n---\n([\s\S]*)$/);
    const get = (k) => front.match(new RegExp(`^(?:  )?${k}: (.*)$`, 'm'))?.[1] ?? '';
    // The install tabs: each channel becomes a bold label.
    const lines = body.replace(/^import .*\n/gm, '').replace(/<RuleSearch \/>\n?/, '')
      .replace(/<InstallTabs [^>]*\/>/, installMarkdown());
    twin(slug, { title: get('title'), description: get('description') }, extra[slug] ?? lines);
  }
}

const page = (slug) => twins.get(slug);
const md = (slug) => `${origin}/${slug}.md`;
const render = (slug) => {
  const p = page(slug);
  return `# ${p.title}\n\n${p.description ? `> ${p.description}\n\n` : ''}${p.text}\n`;
};

export function write(pub) {
  // Twins live beside the site's own files; the page `rules/x` and the index `rules` do not collide.
  for (const d of ['languages', 'compare', 'reference']) rmSync(pub + d, { recursive: true, force: true });
  for (const slug of twins.keys()) {
    mkdirSync(dirname(pub + slug), { recursive: true });
    writeFileSync(`${pub}${slug}.md`, render(slug));
  }
  copyFileSync(rulesPath, pub + 'rules.json');

  const { languages, tools } = siteData();
  const list = (slugs) => slugs.map((s) => `- [${page(s).title}](${md(s)}): ${page(s).description}`).join('\n');
  const guide = ['getting-started', 'cli', 'languages', 'compare', 'reference/presets', 'reference/format-options', 'rules'];
  const llms = `# Sudsy

> ${page('index').description || 'Formatter and linter for many languages in one binary and one config file.'} One \`sudsy.yaml\`, one command (\`sudsy check\`, \`sudsy fix\`). It reproduces the output of tools such as Prettier, ESLint, ruff and hadolint, with a measured match rate per language.

Every page has a Markdown copy at the same URL with \`.md\` added. For one rule, fetch \`${origin}/rules/<rule-id>.md\`.

## Guide

${list(guide)}

## Machine-readable

- [rules.json](${origin}/rules.json): every rule with languages, category, default level, fix kind, options, equivalent ids of other tools and an example
- \`sudsy check --format json\`: diagnostics with rule, range and fix edits; documented in [Command line](${md('cli')})
- \`sudsy init --agents\`: adds a block to \`AGENTS.md\` that tells an agent how to run \`sudsy check\` and \`sudsy fix\`

## Languages

${list(languages.map((l) => `languages/${l.slug}`))}

## Compared tools

Each page lists every rule of the tool, the Sudsy rule that covers it and the share of its findings Sudsy also reports.

${list(tools.map((t) => `compare/${t.tool}`))}

## Optional

- [llms-full.txt](${origin}/llms-full.txt): the guide and language pages in one file
- [Source](https://github.com/bingsubandit/sudsy)
`;
  writeFileSync(pub + 'llms.txt', llms);
  const full = [...guide.filter((s) => s !== 'rules' && s !== 'compare'), ...languages.map((l) => `languages/${l.slug}`)]
    .map((s) => `<!-- ${md(s)} -->\n${render(s)}`);
  writeFileSync(pub + 'llms-full.txt', `# Sudsy: documentation\n\n${full.join('\n---\n\n')}`);
}
