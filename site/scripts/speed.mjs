// Speed comparisons of every language (/speed/) and the claim of the landing page: on how many of the languages
// measured against their primary linter Sudsy lints faster. All of it is computed from docs/ai/parity/site.json.
import { base, siteData } from '../config.mjs';
import { badge } from './badges.mjs';

export const SPEED_SLUG = 'speed';
const first = (name) => name.toLowerCase().split(' ')[0];
const seconds = (cell) => cell.split(' / ').map((s) => parseFloat(s));

// The primary linter of a language is the first tool of its Lint column. A language is counted when its speed
// rows hold a measured time for that tool; every other language is listed with its reason.
export function lintClaim() {
  const faster = [], slower = [], left = [];
  for (const l of siteData().languages) {
    const tool = l.lint[0]?.name;
    if (!tool) { left.push({ language: l.name, reason: 'no reference linter' }); continue; }
    const row = l.speed.find((s) => s[1] === 'Lint' && first(s[2]) === first(tool));
    const [sudsy, other] = row ? seconds(row[4]) : [NaN, NaN];
    if (!(sudsy > 0 && other > 0)) { left.push({ language: l.name, tool, reason: row ? 'not measured' : 'no speed measurement yet' }); continue; }
    (sudsy < other ? faster : slower).push({ language: l.name, slug: l.slug, tool, verdict: row[3] });
  }
  const total = faster.length + slower.length;
  return { faster, slower, left, total, percent: total ? Math.floor((faster.length / total) * 100) : 0 };
}

// "Faster than the primary linter on 85% of measured languages"
export const claimText = (c = lintClaim()) => `Faster than the primary linter on ${c.percent}% of measured languages`;

export function speedPage(put, table, cell) {
  const c = lintClaim();
  const rows = siteData().languages.flatMap((l) => l.speed.map((s) => [l.name, s[1], ...s.slice(2)]));
  const verdict = (v) => (v.includes('slower') ? badge('partial', v) : v.includes('faster') ? badge('covered', v) : v);
  put('speed.md', {
    title: 'Speed comparisons',
    slug: SPEED_SLUG,
    description: 'Wall-clock time and peak memory of Sudsy against the reference tools, for every language measured.',
  }, [
    `${claimText(c)}: ${c.faster.length} of ${c.total}, counting each language's first reference linter. Slower than it: ${c.slower.map((r) => `${r.language} (${r.tool}, ${r.verdict})`).join(', ') || 'none'}. Not counted: ${c.left.map((r) => `${r.language} (${r.reason})`).join(', ') || 'none'}.`,
    'Best of five wall-clock runs of the same task over the same files, a new process each time; the method and the corpus are in [How languages are measured](' + `${base}/measured/` + ').',
    table(['Language', 'Task', 'Compared with', 'Sudsy speed', 'Time (Sudsy / tool)', 'Peak memory (Sudsy / tool)'], rows.map((r) => [r[0], badge('', r[1]), cell(r[2]), verdict(r[3]), cell(r[4]), cell(r[5])])),
  ].join('\n\n'));
}
