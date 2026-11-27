// Writes the data-driven pages (rules, upstream-id stubs, presets, format options) as Markdown into
// src/content/docs/generated/ (git-ignored). Sources: docs/ai/rules/rules.json, schemas/sudsy.schema.json,
// crates/lang-*/presets/*.yaml and the notes column of docs/ai/formatting/presets.md.
import { copyFileSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { base, site, rules } from '../config.mjs';
import { check, names, pages } from './pages.mjs';
import { badge, pageHead } from './badges.mjs';
import { handwritten, twin, write as writeAgentDocs } from './agents.mjs';

const root = fileURLToPath(new URL('../../', import.meta.url));
const out = fileURLToPath(new URL('../src/content/docs/generated/', import.meta.url));
const pub = fileURLToPath(new URL('../public/', import.meta.url));
mkdirSync(pub, { recursive: true });
copyFileSync(root + 'assets/logo/soap_color.svg', pub + 'favicon.svg'); // git-ignored copies
copyFileSync(root + 'assets/logo/og.png', pub + 'og.png');
writeFileSync(pub + 'robots.txt', `User-agent: *\nAllow: /\n\nSitemap: ${site}${base}/sitemap-index.xml\n`);
rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });

const put = (file, front, body, twinBody = body) => {
  twin(front.slug, front, twinBody);
  writeFileSync(out + file, `---\n${Object.entries(front).map(([k, v]) => `${k}: ${JSON.stringify(v)}`).join('\n')}\n---\n\n${body}\n`);
};
// Raw HTML in a description must not open a tag: escape `<` outside code spans; one line for table cells.
const md = (t) => t.split(/(`[^`]*`)/).map((s, i) => (i % 2 ? s : s.replace(/</g, '&lt;'))).join('').replace(/\s+/g, ' ').trim();
// A rule text: paragraphs (separated by a blank line) kept, each on one line.
const prose = (t) => t.split(/\n{2,}/).map(md).filter(Boolean).join('\n\n');
const cell = (t) => md(String(t)).replace(/\|/g, '\\|');
const table = (head, rows) => `| ${head.join(' | ')} |\n|${head.map(() => '---').join('|')}|\n${rows.map((r) => `| ${r.join(' | ')} |`).join('\n')}`;
const code = (lang, text) => '```' + lang + '\n' + text.replace(/\n$/, '') + '\n```';
const url = (slug) => `${base}/rules/${slug}/`;
// Allowed values go under the description, so the table keeps to the text column.
const described = (text, values) => cell(text) + (values?.length ? `<br>Values: ${values.map((c) => `\`${c}\``).join(', ')}` : '');
const fmt = (v) => (v === undefined ? '' : `\`${JSON.stringify(v)}\``);

// Rule config as sudsy.yaml takes it: `lint.rules` at the top level for rules of all languages,
// under the language section otherwise (docs/ai/config/sudsy-yaml.md).
function configuration(r) {
  const langs = r.languages.includes('all') ? [null] : r.languages;
  const indent = (t, n) => t.replace(/^/gm, ' '.repeat(n));
  const yaml = (entry) =>
    langs.map((l) => (l ? `${l}:\n  lint:\n    rules:\n${indent(entry, 6)}` : `lint:\n  rules:\n${indent(entry, 4)}`)).join('\n');
  const opts = r.options.filter((o) => o.default !== undefined);
  const levels = ['off', 'warn', 'error'].map((l) => (l === r.default_level ? `${l} (default)` : l)).join(', ');
  const level = r.default_level === 'off' ? 'warn' : r.default_level;
  // A rule without options is `id: level`; one with options keeps `level` and `options`.
  const entry = opts.length
    ? `${r.id}:\n  level: ${level}  # ${levels}\n  options:\n${opts.map((o) => `    ${o.name}: ${JSON.stringify(o.default)}`).join('\n')}`
    : r.options.length
      ? `${r.id}:\n  level: ${level}  # ${levels}`
      : `${r.id}: ${level}  # ${levels}`;
  return [
    '## Configuration',
    code('yaml', yaml(entry)),
    r.options.length &&
      table(['Option', 'Type', 'Default', 'Description'], r.options.map((o) => [`\`${o.name}\``, o.type, fmt(o.default), described(o.description, o.choices)])),
    r.presets.length && 'Enabled by the presets: ' + r.presets.map((p) => `[\`${p}\`](${base}/reference/presets/#${p.replace('/', '')})`).join(', ') + '.',
    `Suppress one line with \`sudsy-disable-next-line ${r.id}\` in a comment of the file's language.`,
  ].filter(Boolean).join('\n\n');
}

// The example of a page is the window of the rule's own findings: the lines around the reported spans
// (`...` between), without the lines where only other rules of the shared fixture report. The context shrinks
// from 3 to 0 lines, then the last spans are left out, until at most 25 lines remain; the fixed text is cut
// the same way when it has the same line count, else left out.
const MAX_SHOWN = 25;
function cut(e) {
  const lines = e.input.replace(/\n$/, '').split('\n');
  for (let n = e.spans.length; n >= 1; n--) {
    for (const context of [3, 2, 1, 0]) {
      const result = cutAround({ ...e, spans: e.spans.slice(0, n) }, lines, context);
      if (result.lines.length <= MAX_SHOWN) return result;
    }
  }
  throw new Error(`example of ${e.id} cannot be cut to ${MAX_SHOWN} lines`);
}
function cutAround(e, lines, context) {
  const keep = new Set();
  const skip = new Set(e.other_lines ?? []);
  for (const s of e.spans) for (let l = Math.max(1, s.line - context); l <= Math.min(lines.length, s.line + context); l++) keep.add(l);
  const fixedLines = e.fixed?.replace(/\n$/, '').split('\n');
  const map = new Map(); // old line -> new line
  const out = [];
  const fixedOut = [];
  let prev = 0;
  for (let l = 1; l <= lines.length; l++) {
    if (!keep.has(l) || skip.has(l)) continue;
    if (l > prev + 1) { out.push('...'); fixedOut.push('...'); }
    out.push(lines[l - 1]);
    fixedOut.push(fixedLines?.[l - 1]);
    map.set(l, out.length);
    prev = l;
  }
  if (prev < lines.length) { out.push('...'); fixedOut.push('...'); }
  const sameShape = fixedLines && fixedLines.length === lines.length;
  return { lines: out, spans: e.spans.map((s) => ({ ...s, line: map.get(s.line) })), fixed: sameShape ? fixedOut.join('\n') : undefined };
}

function example(e, id) {
  if (!e) return '';
  const { lines, spans, fixed } = cut({ ...e, id });
  const squiggles = spans.map((s) => `${s.line}:${s.column}:${s.length}`).join(',');
  return [
    '## Example',
    e.config ? 'With this configuration:\n\n' + code('yaml', e.config) : '',
    code(e.language, lines.join('\n')).replace(/^```(\S*)/, `\`\`\`$1 title="Reported" squiggles="${squiggles}" tip="${id}"`),
    fixed ? code(e.language, fixed).replace(/^```(\S*)/, '```$1 title="After the fix"') : '',
  ].filter(Boolean).join('\n\n');
}

const { langName, tools: byTool } = names();
const ruleIds =new Set(rules().map((r) => r.id.toLowerCase()));
const index = []; // the rule list of /rules/, read by the browser
const stubs = new Map(); // slug -> { id, tools, rules }
for (const r of rules()) {
  const head = [
    ...r.languages.map((l) => (l === 'all' ? badge('lang', 'all languages') : badge('lang', langName.get(l)?.name ?? l, langName.has(l) && `${base}/languages/${langName.get(l).slug}/`))),
    badge('category', r.category),
    badge(`fix ${r.fix ?? 'none'}`, r.fix ? `fix: ${r.fix}` : 'no fix'),
    badge(`level ${r.default_level}`, `default: ${r.default_level}`),
    ...r.presets.map((p) => badge('preset', p, `${base}/reference/presets/#${p.replace('/', '')}`)),
  ];
  const anyPartial = r.upstream.some((u) => u.partial);
  const upstreamTable = r.upstream.length &&
    '## Upstream equivalents\n\n' +
      (anyPartial ? 'The same check in other tools. `full`: Sudsy reports what the upstream rule reports; `partial`: only part of it, as the last column says.\n\n'
        : 'The same check in other tools; Sudsy reports what each upstream rule reports.\n\n') +
      table(['Tool', 'Rule', 'Match', ...(anyPartial ? ['What differs'] : [])], r.upstream.map((u) => {
        const t = byTool.get(u.tool.toLowerCase());
        return [t ? `[${u.tool}](${base}/compare/${t.tool}/)` : u.tool, u.url ? `[\`${u.id}\`](${u.url})` : `\`${u.id}\``,
          badge(u.partial ? 'partial' : 'covered', u.partial ? 'partial' : 'full'), ...(anyPartial ? [cell(u.note ?? '')] : [])];
      }));
  const why = r.why ? `## Why it matters\n\n${prose(r.why)}` : '';
  const parts = [`## What it does\n\n${prose(r.description)}`, why, `<div class="example">\n\n${example(r.example, r.id)}\n\n</div>`, configuration(r), upstreamTable];
  const body = [pageHead(head), ...parts].filter(Boolean).join('\n\n');
  const plain = `Languages: ${r.languages.join(', ')}. Category: ${r.category}. Fix: ${r.fix ?? 'none'}. Default level: ${r.default_level}.`;
  const twinBody = [prose(r.description), why, plain, ...parts.slice(2)].filter(Boolean).join('\n\n');
  index.push({
    i: r.id, l: r.languages, c: r.category, f: r.fix ?? 'none', p: r.presets,
    u: r.upstream.map((u) => [u.tool, u.id, u.url ?? '']),
    d: md(r.description.split('\n')[0]).split(/(?<=\.)\s/)[0],
  });
  const langs = r.languages.includes('all') ? 'all languages' : r.languages.map((l) => langName.get(l)?.name ?? l).join(', ');
  const like = [...new Set(r.upstream.map((u) => `${u.tool} ${u.id}`))].slice(0, 3);
  put(`rule-${r.id}.md`, {
    title: r.id, slug: `rules/${r.id}`, description: md(r.description.split(/\n{2,}/)[0]).split(/(?<=\.)\s/)[0].slice(0, 160),
    head: [{ tag: 'title', content: `${r.id}: ${langs} lint rule${like.length ? `, like ${like.join(', ')}` : ''} | Sudsy` }],
  }, body, twinBody);

  for (const u of r.upstream) {
    const slug = u.id.replace(/[^A-Za-z0-9_-]+/g, '-').replace(/^-+|-+$/g, '');
    if (!slug || ruleIds.has(slug.toLowerCase())) continue;
    const s = stubs.get(slug.toLowerCase()) ?? { slug, id: u.id, tools: new Set(), rules: new Set() };
    s.tools.add(u.tool);
    s.rules.add(r.id);
    stubs.set(slug.toLowerCase(), s);
  }
}

// One Sudsy rule: redirect to it. Several: list them (a redirect would hide the others).
// Plain static files in public/ (a Starlight page would carry 19 KB of layout each).
rmSync(pub + 'rules', { recursive: true, force: true });
for (const s of stubs.values()) {
  const ids = [...s.rules];
  const tools = [...s.tools].join(', ');
  const head = ids.length === 1
    ? `<meta http-equiv="refresh" content="0; url=${url(ids[0])}"><link rel="canonical" href="${site + url(ids[0])}">`
    : '';
  mkdirSync(`${pub}rules/${s.slug}`, { recursive: true });
  writeFileSync(`${pub}rules/${s.slug}/index.html`,
    `<!doctype html><meta charset="utf-8"><title>${s.id} | Sudsy</title>${head}<body>${tools} <code>${s.id}</code> is covered by ${ids.map((i) => `<a href="${url(i)}">${i}</a>`).join(', ')}.\n`);
}

// The rule list of /rules/: one file, fetched once and filtered in the browser (no request per rule).
const languageNames = {};
for (const l of new Set(rules().flatMap((r) => r.languages))) {
  if (l === 'all') continue;
  // A page of several languages ("INI / EditorConfig / .gitignore") names each id by its own part, so filters stay distinct.
  const key = (t) => t.toLowerCase().replaceAll('+', 'p').replace(/[^a-z0-9]/g, '');
  const page = langName.get(l)?.name ?? l;
  languageNames[l] = [page.split(' / ').find((part) => key(part) === key(l)) ?? page, langName.get(l)?.slug ?? l];
}
writeFileSync(pub + 'rules-index.json', JSON.stringify({ languages: languageNames, rules: index }));

// Presets: the notes of docs/ai/formatting/presets.md next to each preset file.
const notes = new Map();
for (const m of readFileSync(root + 'docs/ai/formatting/presets.md', 'utf8').matchAll(/^\| `([a-z]+\/[\w-]+)` \| (.*) \|$/gm)) {
  notes.set(m[1], m[2].replace(/\[([^\]]*)\]\([^)]*\)/g, '$1'));
}
const presets = [];
for (const dir of readdirSync(root + 'crates').filter((d) => existsSync(`${root}crates/${d}/presets`))) {
  for (const f of readdirSync(`${root}crates/${dir}/presets`)) {
    presets.push([`${dir.replace('lang-', '')}/${f.replace('.yaml', '')}`, readFileSync(`${root}crates/${dir}/presets/${f}`, 'utf8')]);
  }
}
put('presets.md', { title: 'Presets', slug: 'reference/presets', description: 'Built-in presets for `extends: [preset:<language>/<name>]`.' },
  'A preset is an ordinary `sudsy.yaml`; writing its options by hand gives the same result. Use one with `extends: [preset:<language>/<name>]`.\n\n' +
    presets.sort(([a], [b]) => a.localeCompare(b)).map(([name, yaml]) =>
      `## ${name}\n\n${md(notes.get(name) ?? '')}\n\n[Rules it enables](${base}/rules/?preset=${name})\n\n${code('yaml', yaml)}`).join('\n\n'));

// Format options: every language's `format` section of the schema.
const schema = JSON.parse(readFileSync(root + 'schemas/sudsy.schema.json', 'utf8'));
const sections = Object.entries(schema.properties)
  .filter(([, v]) => v.properties?.format)
  .map(([lang, v]) => [lang, v.properties.format.properties]);
put('format-options.md', { title: 'Format options', slug: 'reference/format-options', description: 'Every formatting option per language, with its type, default and allowed values.' },
  'Set under `<language>.format` in `sudsy.yaml`. The options `indent_style`, `indent_size`, `line_width`, `end_of_line`, `final_newline`, `bracket_spacing` and `array_bracket_spacing` can also be set once under a top-level `format`.\n\n' +
    sections.map(([lang, props]) =>
      `## ${lang}\n\n` + table(['Option', 'Type', 'Default', 'Description'],
        Object.entries(props).sort(([a], [b]) => a.localeCompare(b)).map(([n, o]) =>
          [`\`${n}\``, o.type, fmt(o.default), described(o.description ?? '', o.enum ?? (o.minimum !== undefined ? [`${o.minimum}..${o.maximum ?? ''}`] : []))]))).join('\n\n'));

check(out, pages({ put, cell, table }));
handwritten(fileURLToPath(new URL('../src/content/docs/', import.meta.url)));
writeAgentDocs(pub);
console.log(`generated ${rules().length} rules, ${stubs.size} upstream stubs, ${presets.length} presets`);
