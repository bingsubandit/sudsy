// The numbers and texts of the landing page, all read from generated data (docs/ai/parity/site.json, rules.json)
// or from the README; the components in src/components/landing/ only lay them out.
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { rules, siteData } from '../config.mjs';

const readme = readFileSync(resolve(process.cwd(), '../README.md'), 'utf8');

// The before / after CI step of the README's Why section.
export const beforeAfter = () => readme.slice(readme.indexOf('## Why')).match(/```diff\n([\s\S]*?)```/)[1].trimEnd();

// The early-release note of the README, as plain text.
export const earlyRelease = () => readme.match(/\*\*Early release \(([^)]*)\)\.\*\*/)[1];

export const isReady = (l) => /Ready$/.test(l.status);

// Ready and in-progress counts of the support table.
export function counts() {
  const { languages } = siteData();
  const ready = languages.filter(isReady).length;
  const presets = new Set(languages.flatMap((l) => l.presets));
  const safe = rules().filter((r) => r.fix === 'safe').length;
  return { languages: languages.length, ready, progress: languages.length - ready, presets: presets.size, rules: rules().length, safe };
}

// "0.21 s / 4.87 s" -> [0.21, 4.87]
const seconds = (cell) => cell.split(' / ').map((s) => parseFloat(s));

// The speed bars: the primary reference tool (first of the Format and of the Lint column) of the first
// `limit` most popular languages of the support table that have a measured speed row for it; /speed/ lists every row.
export function speedRows(limit = 5) {
  const first = (name) => name.toLowerCase().split(' ')[0];
  const out = [];
  for (const l of siteData().languages) {
    if (new Set(out.map((r) => r.language)).size >= limit) break;
    for (const [task, list] of [['Format', l.format], ['Lint', l.lint]]) {
      const tool = list[0]?.name;
      const row = tool && l.speed.find((s) => s[1] === task && first(s[2]) === first(tool));
      if (!row) continue;
      const [sudsy, other] = seconds(row[4]);
      const [sudsyText, otherText] = row[4].split(' / ');
      out.push({ language: l.name, slug: l.slug, task, tool: row[2], verdict: row[3], sudsy, other, sudsyText, otherText });
    }
  }
  return out;
}
