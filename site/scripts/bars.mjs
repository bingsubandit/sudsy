// The progress bar of a measured share, plain HTML and CSS (styles: src/styles/site.css, `.bar`): filled to the
// floored percent the README shows, a tick at the Ready bar, and the state readable without colour (number,
// tick, "not measured"). One bar per reference tool; the language pages, the language index and the tool
// index use it.
import { base } from '../config.mjs';
import { esc } from './badges.mjs';

export const READY = 85;

// `what` completes the label: "Format: 95% of files identical to Prettier".
export function bar({ share, name, href, what, label: given, primary = false }) {
  const measured = share != null;
  const pct = measured ? Math.floor(share) : 0;
  const state = !measured ? 'none' : pct >= READY ? 'ok' : 'low';
  const label = given ?? `${what.task}: ${measured ? `${pct}% of ${what.of}` : 'not measured'}${name ? ` ${what.prep} ${name}` : ''}`;
  const tool = name ? (href ? `<a class="bar-tool" href="${href}">${esc(name)}</a>` : `<span class="bar-tool">${esc(name)}</span>`) : '';
  return `<span class="bar ${state}${primary ? ' primary' : ''}">${tool}<span class="bar-track" role="img" aria-label="${esc(label)}"><span class="bar-fill" style="width:${pct}%"></span><span class="bar-tick"></span></span><span class="bar-num">${measured ? `${pct}%` : 'not measured'}</span></span>`;
}

export const FORMAT = { task: 'Format', of: 'files identical', prep: 'to' };
export const LINT = { task: 'Lint', of: 'findings also reported', prep: 'with' };

// The bars of one language: a Format and a Lint group, the first tool of each group primary. A group
// without a reference tool shows its text instead of a bar.
export function legs(l, toolsWithPage) {
  const href = (x) => (x.catalog && toolsWithPage.has(x.catalog) ? `${base}/compare/${x.catalog}/` : undefined);
  const group = (title, list, what, empty) =>
    `<div class="leg"><span class="leg-title">${title}</span>${list.length
      ? `<div class="leg-bars">${list.map((x, i) => bar({ share: x.share, name: x.name, href: href(x), what, primary: i === 0 })).join('')}</div>`
      : `<span class="leg-empty">${empty}</span>`}</div>`;
  return group('Format', l.format, FORMAT, 'no reference tool') + group('Lint', l.lint, LINT, `${l.rules} rules, no reference tool`);
}
