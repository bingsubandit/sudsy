// Language pages (/languages/<id>/), tool pages (/compare/<tool>/) and the check that every rule, language
// and tool has a page. Source: docs/ai/parity/site.json (cargo xtask codegen; the README support table,
// the speed table and the parity catalogs), next to rules.json.
import { existsSync } from 'node:fs';
import { base, rules, siteData } from '../config.mjs';
import { badge, languageBadges, pageHead, statusBadge } from './badges.mjs';
import { bar, legs, LINT, READY } from './bars.mjs';
import { speedPage } from './speed.mjs';
import { SLUG as MEASURED, measuredLink, measuredPage } from './measured.mjs';

const pct = (v) => (v == null ? 'not measured' : `${Math.floor(v)}%`);
const link = (path, text) => `[${text}](${base}/${path}/)`;
const tick = (t) => '`' + t + '`';

// Names shown for a tool: the form rules.json uses (`StyleCop`), else the catalog id.
export function names() {
  const { languages, tools } = siteData();
  const toolName = new Map(tools.map((t) => [t.tool, t.tool]));
  for (const r of rules()) for (const u of r.upstream) toolName.set(u.tool.toLowerCase(), u.tool);
  const langName = new Map(languages.flatMap((l) => l.ids.map((i) => [i, l])));
  return { toolName, langName, tools: new Map(tools.map((t) => [t.tool, t])) };
}

// What Sudsy has for one catalog entry: `[id, coverage, status, gap]`.
function sudsy([, coverage, status, gap]) {
  const [kind, arg] = [status.split(': ')[0], status.split(': ').slice(1).join(': ')];
  const what = {
    rule: () => arg.split(', ').map((r) => link(`rules/${r}`, tick(r))).join(', '),
    option: () => `option ${tick(arg)}`,
    sdk: () => `.NET SDK ${tick(arg)}`,
    config: () => `configuration: ${arg}`,
    wontfix: () => '-',
    planned: () => '-',
    deferred: () => '-',
    formatter: () => 'formatter',
  }[kind]();
  const label = { covered: 'covered', partial: 'partial', missing: 'not yet', sdk: 'SDK', wontfix: 'not planned', deferred: 'deferred' }[coverage];
  return [what, badge(coverage, label), kind === 'wontfix' ? arg : gap];
}

// "26 of 29 languages are ready, 2 are in progress and 1 is not measured yet." (exact counts from the data)
function summary(languages) {
  const n = (f) => languages.filter(f).length;
  const [ready, progress, none] = [n((l) => /Ready$/.test(l.status)), n((l) => l.status === 'In progress'), n((l) => l.status === 'not measured')];
  const is = (k) => (k === 1 ? 'is' : 'are');
  return `${ready} of ${languages.length} languages are ready, ${progress} ${is(progress)} in progress and ${none} ${is(none)} not measured yet. \`Ready\` means the first tool of each column is at least ${READY}%; the others are shown for reference. Each bar is a share measured on real-world code against the default rule set of the reference tool (a language measured with another set says so on its page); the tick marks ${READY}%. Ready languages come first, then the others, each by how widely the language is used. [How each language is measured](${base}/${MEASURED}/).`;
}

// The shares measured for one tool: its lint share (the parity target) and the format share of each language
// that names it as a reference formatter.
function measured(t, languages) {
  const format = languages.flatMap((l) => l.format.filter((x) => x.catalog === t.tool && x.share != null).map((x) => ({ language: l.name, tool: x.name, share: x.share })));
  return { lint: t.target, format };
}

export function pages({ put, cell, table }) {
  const { languages, tools } = siteData();
  const { toolName, langName, tools: byTool } = names();
  const show = (t) => toolName.get(t) ?? t;
  const written = { languages: [], tools: [] };

  for (const t of tools) {
    const n = (c) => t.entries.filter((e) => e[1] === c).length;
    const langs = t.languages.map((l) => langName.get(l)).filter(Boolean);
    const m = measured(t, languages);
    const shares = [
      ...(m.lint == null ? [] : [badge(m.lint >= READY ? 'covered' : 'partial', `${pct(m.lint)} of its findings also reported`)]),
      ...m.format.map((f) => badge(f.share >= READY ? 'covered' : 'partial', `${pct(f.share)} of ${f.language} files identical`)),
    ];
    const share = shares.length ? shares.join('') : badge('missing', 'not measured on a corpus');
    const head = pageHead([
      ...languageBadges(langs),
      ...(t.version ? [badge('version', `version ${t.version}`)] : []),
      share,
      badge('covered', `${n('covered')} covered`),
      badge('partial', `${n('partial')} partial`),
      badge('missing', `${n('missing')} planned`),
      badge('deferred', `${n('deferred')} deferred`),
    ]);
    const body = [
      head,
      `Every rule and setting of ${show(t.tool)} in the parity catalog, with the Sudsy equivalent. ${m.lint == null ? '' : `The lint share is the part of ${show(t.tool)}'s default set (in scope) whose findings Sudsy also reports on real-world code, with the preset that mirrors it. `}${m.format.length ? `The format share is the part of real-world files that Sudsy formats to the same text as ${show(t.tool)}. ` : ''}Where the tool has a page for an entry, the id links to it.`,
      "**Status**: `covered`; `partial` (the note says what differs); `not yet` (planned); `deferred`; `not planned`; `SDK` (a .NET SDK diagnostic does it).",
      '<div class="tool-table">\n\n' + table(['Id', 'Sudsy', 'Status', 'Note'], t.entries.map((e) => {
        const id = tick(e[0]).replace(/\|/g, '\\|');
        const [what, status, note] = sudsy(e);
        return [e[4] ? `[${id}](${e[4]})` : id, cell(what), status, cell(note)];
      })) + '\n\n</div>',
    ].filter(Boolean).join('\n\n');
    put(`compare-${t.tool}.md`, {
      title: `${show(t.tool)} rules in Sudsy`,
      slug: `compare/${t.tool}`,
      description: `Every ${show(t.tool)} rule with the Sudsy rule that covers it and how much of ${show(t.tool)}'s output Sudsy reproduces.`,
      head: [{ tag: 'title', content: `${show(t.tool)} rules and their Sudsy equivalents | Sudsy` }],
    }, body);
    written.tools.push(t.tool);
  }
  put('compare-index.md', {
    title: 'Compared tools', slug: 'compare',
    description: 'The linters and formatters Sudsy is measured against, with the share of their output it reproduces.',
  }, 'Every tool Sudsy is measured against: its rules and settings, the Sudsy rule for each, and the share of its findings Sudsy also reports.\n\n' +
    table(['Tool', 'Entries', 'Share Sudsy reproduces', 'Languages'], tools.map((t) => {
      const m = measured(t, languages);
      const lint = m.lint == null ? [] : [bar({ share: m.lint, name: m.format.length ? 'Lint' : '', what: LINT, label: `Lint: ${Math.floor(m.lint)}% of ${show(t.tool)} findings also reported` })];
      const format = m.format.map((f) => bar({ share: f.share, name: f.language, what: { task: 'Format', of: 'files identical', prep: 'to' + ` ${f.tool} for` } }));
      const bars = [...lint, ...format];
      return [link(`compare/${t.tool}`, show(t.tool)), t.entries.length, bars.length ? `<div class="leg-bars">${bars.join('')}</div>` : bar({ share: null, what: { task: show(t.tool), of: '' } }), languageBadges(t.languages.map((l) => langName.get(l)).filter(Boolean)).join(' ')];
    })));

  const task = (t) => badge('', t);
  for (const l of languages) {
    const parity = l.format.length + l.lint.length;
    const notes = [...l.format, ...l.lint].filter((x) => x.note).map((x) => `${x.name}: ${x.note}`);
    const body = [
      pageHead([statusBadge(l.status), badge('rules', `${l.rules} lint rules`)]),
      `Each bar is measured on real-world code against the default rule set of the reference tool${notes.length ? `, except ${notes.join('; ')}` : ''}; the tick marks ${READY}%. \`Ready\` means the first tool of each column is at least ${READY}%; the others are shown for reference. [How ${l.name} is measured](${measuredLink(l.slug)}).`,
      l.requires,
      '## Parity',
      parity ? `<div class="legs">${legs(l, byTool)}</div>` : `Sudsy has ${l.rules} rules for this language; no reference tool was found to compare with.`,
      l.speed.length && '## Speed\n\n' + table(['Task', 'Compared with', 'Sudsy speed', 'Time (Sudsy / tool)', 'Peak memory (Sudsy / tool)'], l.speed.map((s) => [task(s[1]), ...s.slice(2).map(cell)])),
      l.presets.length && '## Presets\n\nA preset is an ordinary `sudsy.yaml`; use one with `extends`.\n\n' + l.presets.map((p) => `- [${tick(p)}](${base}/reference/presets/#${p.replace('/', '')}): \`extends: [preset:${p}]\``).join('\n'),
      `## Rules\n\n${l.rules} lint rules apply to ${l.name}: [browse them](${base}/rules/?language=${l.ids[0]}).`,
    ].filter(Boolean).join('\n\n');
    const against = [...new Set([...l.format, ...l.lint].map((x) => x.name))];
    put(`language-${l.slug}.md`, {
      title: l.name,
      slug: `languages/${l.slug}`,
      description: `Sudsy formats and lints ${l.name}: ${l.status.toLowerCase()}${against.length ? `, measured against ${against.join(', ')}` : ''}.`,
      head: [{ tag: 'title', content: `${l.name} formatter and linter${against.length ? ` (${against.join(', ')} parity)` : ''} | Sudsy` }],
    }, body);
    written.languages.push(l.slug);
  }
  put('language-index.md', {
    title: 'Languages', slug: 'languages',
    description: 'Every language Sudsy formats and lints, with its status against the reference tools.',
  }, [
    summary(languages),
    '<div class="lang-list">' + languages.map((l) => `<div class="lang-row"><div class="lang-head"><a href="${base}/languages/${l.slug}/">${l.name}</a>${statusBadge(l.status)}</div>${legs(l, byTool)}<a class="lang-how" href="${measuredLink(l.slug)}">How ${l.name} is measured</a></div>`).join('') + '</div>',
  ].join('\n\n'));
  written.measured = measuredPage(put);
  speedPage(put, table, cell);
  return written;
}

// The pages exist: one per rule, per language of a rule, per tool.
export function check(out, written) {
  const { languages, tools } = siteData();
  const missing = [
    ...rules().filter((r) => !existsSync(`${out}rule-${r.id}.md`)).map((r) => `rule ${r.id}`),
    ...rules().flatMap((r) => r.languages.filter((l) => l !== 'all' && !languages.some((x) => x.ids.includes(l))).map((l) => `language ${l} (rule ${r.id})`)),
    ...languages.filter((l) => !written.languages.includes(l.slug)).map((l) => `language ${l.slug}`),
    ...tools.filter((t) => !written.tools.includes(t.tool)).map((t) => `tool ${t.tool}`),
  ];
  if (missing.length) throw new Error(`no page for: ${missing.join(', ')}`);
}
