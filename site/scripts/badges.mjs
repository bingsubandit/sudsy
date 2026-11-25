// The badge markup shared by the generated pages: the header block of rule, language and tool pages
// (`<div class="page-head">`) and the status cells of tables. Styles: src/styles/site.css (`.badge`).
import { base } from '../config.mjs';

export const esc = (t) => String(t).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/"/g, '&quot;');
export const badge = (kind, label, href) => `<span class="badge ${kind}">${href ? `<a href="${href}">${esc(label)}</a>` : esc(label)}</span>`;

// One linked badge per language (a language of the support table: `{ name, slug }`), each language once.
export const languageBadges = (languages) =>
  [...new Map(languages.map((l) => [l.slug, l])).values()].map((l) => badge('lang', l.name, `${base}/languages/${l.slug}/`));

// The status badge of a language row of the support table (`Ready`, `Lint only: Ready`, `In progress`, `not measured`).
export const statusBadge = (st) => badge(/Ready$/.test(st) ? 'covered' : st === 'not measured' ? 'missing' : 'partial', st);

export const pageHead =(badges) => `<div class="page-head">${badges.join('')}</div>`;
