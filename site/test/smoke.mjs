// Browser smoke test of the built site (site/dist): `scripts/site.sh test`. Needs Edge or Chrome installed,
// or a Playwright Chromium (PLAYWRIGHT_BROWSERS_PATH).
import { after, before, test } from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { chromium } from 'playwright-core';
import { channels } from '../scripts/install.mjs';
import { beforeAfter, counts, speedRows } from '../scripts/landing.mjs';
import { claimText, lintClaim } from '../scripts/speed.mjs';

// SITE_PORT: another checkout may be serving on 4399; SITE_NO_SERVER tests a server that is already running.
const PORT = Number(process.env.SITE_PORT ?? 4399);
const B = `http://localhost:${PORT}/sudsy`;
if (!process.env.SITE_NO_SERVER) spawn('npx', ['astro', 'preview', '--port', String(PORT)], { shell: true, stdio: 'ignore' });
let browser, page, index;
const problems = [];

before(async () => {
  for (let i = 0; i < 100; i++) {
    if (await fetch(`${B}/`).then((r) => r.ok, () => false)) break;
    await new Promise((r) => setTimeout(r, 200));
  }
  for (const options of [{ channel: 'msedge' }, { channel: 'chrome' }, {}]) {
    try { browser = await chromium.launch(options); break; } catch {}
  }
  assert.ok(browser, 'no Edge, Chrome or Playwright Chromium could be launched');
  index = await (await fetch(`${B}/rules-index.json`)).json();
  page = await browser.newPage();
  page.on('pageerror', (e) => problems.push(e.message));
  page.on('console', (m) => m.type() === 'error' && problems.push(m.text()));
  page.on('response', (r) => r.status() >= 400 && problems.push(`${r.status()} ${r.url()}`));
});
after(async () => {
  await browser?.close();
  if (!process.env.SITE_NO_SERVER) spawn('npx', ['astro', 'preview', 'stop'], { shell: true, stdio: 'ignore' }); // astro preview runs as a daemon
});

// "N of M rules" with every one of the N rows rendered.
const settled = () => page.waitForFunction(() => {
  const m = /^(\d+) of \d+ rules/.exec(document.querySelector('#rule-search .count')?.textContent ?? '');
  return m && document.querySelector('#rule-search')?.dataset.rendered === m[1] && document.querySelectorAll('#rule-search tbody tr').length === +m[1];
});
const hits = async () => { await settled(); return parseInt(await page.locator('#rule-search .count').innerText()); };
const open = async (query) => { await page.goto(`${B}/rules/${query}`); return hits(); };
const menu = (title) => page.locator('.rs-menu', { has: page.locator('button', { hasText: new RegExp(`^${title}`) }) });
const noOverflow = async (what) => {
  const [scroll, client] = await page.evaluate(() => [document.documentElement.scrollWidth, document.documentElement.clientWidth]);
  assert.ok(scroll <= client, `${what}: scrollWidth ${scroll} > clientWidth ${client}`);
};

test('the list shows every rule, none cut off', async () => {
  const n = await open('');
  assert.equal(n, index.rules.length);
  assert.ok(n > 50);
  assert.equal(await page.locator('#rule-search tbody tr').count(), n);
  const first = page.locator('#rule-search tbody tr').first();
  assert.ok(await first.locator('th a').count() === 1 && await first.locator('.badge.category').count() === 1);
  for (const h of ['Rule', 'Languages', 'Category', 'Fix', 'Upstream', 'Description']) {
    assert.equal(await page.locator('#rule-search thead th', { hasText: new RegExp(`^${h}$`) }).count(), 1, h);
  }
});

test('a search term matches ids, descriptions and upstream ids; links work', async () => {
  const all = await open('');
  await page.fill('#rule-search .rs-q', 'trailing');
  await page.waitForFunction((n) => parseInt(document.querySelector('#rule-search .count').textContent) < n, all);
  assert.ok(await hits() > 0);
  const link = page.locator('#rule-search tbody th a').first();
  const title = await link.innerText();
  await link.click();
  await page.waitForURL(/\/rules\/[^/]+\/$/);
  assert.equal(await page.locator('h1').innerText(), title);
  assert.ok(await open('?q=SA1005') > 0);
  assert.equal(await page.locator('#rule-search tbody th a', { hasText: /^comment-space$/ }).count(), 1);
});

test('each filter narrows the results; chips remove them; clear resets', async () => {
  const all = await open('');
  for (const [title, name] of [['Language', 'language'], ['Category', 'category'], ['Fix', 'fix'], ['Preset', 'preset'], ['Upstream tool', 'upstream']]) {
    const m = menu(title);
    await m.locator('button').first().click();
    assert.equal(await m.locator('button').first().getAttribute('aria-expanded'), 'true', title);
    const options = await m.locator('label').allTextContents();
    const smallest = options.map((t) => /^\s*(.*?)\s*\((\d+)\)\s*$/.exec(t)).filter((x) => +x[2] > 0).sort((a, b) => a[2] - b[2])[0];
    assert.ok(+smallest[2] < all, `${name}: ${smallest[0]}`);
    await m.locator('label').filter({ hasText: smallest[0].trim() }).first().locator('input').check();
    await page.waitForFunction((n) => parseInt(document.querySelector('#rule-search .count').textContent) < n, all);
    assert.ok(await hits() >= +smallest[2], name);
    assert.equal(await page.locator('.rs-chip').count(), 1);
    await page.keyboard.press('Escape'); // the open menu would cover the chip
    await page.locator('.rs-chip').click(); // remove
    await page.waitForFunction((n) => parseInt(document.querySelector('#rule-search .count').textContent) === n, all);
  }
  await menu('Fix').locator('button').first().click();
  await menu('Fix').locator('label').filter({ hasText: /^\s*safe/ }).locator('input').check();
  await page.locator('.rs-clear').click();
  assert.equal(await hits(), all);
  assert.equal(await page.locator('.rs-chip').count(), 0);
});

test('menus work by keyboard and the long ones have a find field', async () => {
  await open('');
  const lang = menu('Language');
  await lang.locator('button').first().focus();
  await page.keyboard.press('Enter');
  assert.equal(await lang.locator('button').first().getAttribute('aria-expanded'), 'true');
  const find = lang.locator('input[type=search]');
  await find.fill('c#');
  assert.equal(await lang.locator('li:not([hidden])').count(), 1);
  await page.keyboard.press('Escape');
  assert.equal(await lang.locator('button').first().getAttribute('aria-expanded'), 'false');
});

test('query parameters preselect', async () => {
  const all = await open('');
  assert.ok(await open('?language=csharp') < all);
  assert.equal(await page.locator('.rs-chip').count(), 1);
  assert.equal(await page.locator('#rule-search tbody th a', { hasText: /^comment-space$/ }).count(), 1); // a rule for all languages
  assert.ok(await open('?preset=yaml/yamllint') > 0);
  assert.ok(await open('?category=style&fix=safe&upstream=hadolint') >= 0);
  await open('?q=SA1005');
  assert.equal(await page.inputValue('#rule-search .rs-q'), 'SA1005');
});

test('the sidebar has one Rules entry and there is no level filter', async () => {
  await open('');
  const links = page.locator('nav.sidebar a[href*="/rules"]');
  assert.equal(await links.count(), 1);
  assert.equal(await links.first().getAttribute('aria-current'), 'page');
  assert.equal(await page.locator('.rs-menu button', { hasText: /^level/i }).count(), 0);
});

test('rule page: header badges, short config, options config, upstream links, short example', async () => {
  await page.goto(`${B}/rules/apk-pin-versions/`);
  assert.ok(await page.locator('.page-head .badge').count() >= 4);
  assert.equal(await page.getByText('Languages:').count(), 0);
  assert.equal(await page.locator('.page-head .badge.lang a').count(), 1);
  assert.equal(await page.getByRole('heading', { name: 'What it does' }).count(), 1);
  assert.equal(await page.getByRole('heading', { name: 'Why it matters' }).count(), 1);
  const second = page.locator('main p', { hasText: 'A pinned build is repeatable' }); // the paragraphs of a text stay apart
  assert.equal(await second.count(), 1);
  assert.ok(!(await second.textContent()).includes('Without a version'));
  assert.equal(await page.locator('pre', { hasText: 'apk-pin-versions: warn  # off (default), warn, error' }).count(), 1);
  assert.equal(await page.locator('pre', { hasText: 'level:' }).count(), 0);
  assert.equal(await page.getByText('Measured for the tool').count(), 0);
  assert.equal(await page.locator('main th', { hasText: /^Match$/ }).count(), 1);
  assert.equal(await page.locator('main a[href="https://github.com/hadolint/hadolint/wiki/DL3018"]').count(), 1);
  await page.goto(`${B}/rules/sorted-keys/`);
  assert.equal(await page.locator('pre', { hasText: '# off (default), warn, error' }).filter({ hasText: 'options:' }).count(), 1);
  assert.ok(await page.locator('main table .badge.partial').count() >= 1);
  assert.ok((await page.locator('main table tbody td', { hasText: /\w{12}/ }).count()) > 0);
  await page.goto(`${B}/rules/key-name-casing/`);
  assert.equal(await page.locator('.example .squiggle').count(), 3);
  assert.equal(await page.locator('.example .squiggle').first().getAttribute('title'), 'key-name-casing');
  assert.equal(await page.locator('.example pre', { hasText: '^^^' }).count(), 0);
  await page.goto(`${B}/rules/unused-parameter/`); // a 58-line fixture with spread spans is cut around the reported lines
  assert.ok(await page.locator('.example .squiggle').count() >= 1);
  const lines = await page.locator('.example pre').first().locator('.ec-line').count();
  assert.ok(lines <= 30, `example has ${lines} lines`);
  await page.goto(`${B}/rules/comment-space/`);
  await page.goto(`${B}/rules/SA1005/`);
  await page.waitForURL(`${B}/rules/comment-space/`);
});

test('each reported character gets one underline', async () => {
  // a span over several highlighted tokens is one background wave, not a decoration per token
  await page.goto(`${B}/rules/apk-pin-versions/`);
  const one = page.locator('.example .squiggle');
  assert.equal(await one.count(), 1);
  assert.equal(await one.evaluate((e) => getComputedStyle(e).textDecorationLine), 'none');
  assert.notEqual(await one.evaluate((e) => getComputedStyle(e).backgroundImage), 'none');
  // overlapping spans of a line (use-isnan) are merged: no nesting, no two underlines on the same text
  await page.goto(`${B}/rules/use-isnan/`);
  const overlap = await page.evaluate(() => {
    const els = [...document.querySelectorAll('.example .squiggle')];
    const nested = els.filter((e) => e.parentElement.closest('.squiggle')).length;
    let overlaps = 0;
    for (const [i, a] of els.entries()) {
      for (const b of els.slice(i + 1)) {
        const x = a.getBoundingClientRect(), y = b.getBoundingClientRect();
        if (Math.abs(x.top - y.top) < 4 && x.left < y.right - 1 && y.left < x.right - 1) overlaps++;
      }
    }
    return { count: els.length, nested, overlaps };
  });
  assert.ok(overlap.count >= 3, 'use-isnan has underlines');
  assert.deepEqual([overlap.nested, overlap.overlaps], [0, 0]);
});

test('tool page links ids to the tool and labels the status', async () => {
  await page.goto(`${B}/compare/hadolint/`);
  assert.equal(await page.locator('main th', { hasText: /^Status$/ }).count(), 1);
  assert.ok(await page.locator('main a[href="https://github.com/hadolint/hadolint/wiki/DL3008"]').count() >= 1);
  assert.ok(await page.locator('main .badge.covered').count() > 5);
});

// The format shares of a tool: the languages that name it as a reference formatter.
const formatShares = (data, t) => data.languages.flatMap((l) => l.format.filter((x) => x.catalog === t.tool && x.share != null).map((x) => ({ language: l.name, share: x.share })));
const siteData = () => JSON.parse(readFileSync(new URL('../../docs/ai/parity/site.json', import.meta.url), 'utf8'));

test('a tool with a measured share (lint or format) never says it was not measured', async () => {
  const data = siteData();
  assert.ok(data.tools.some((t) => t.target != null));
  assert.ok(data.tools.some((t) => t.target == null && formatShares(data, t).length), 'a formatter with only a format share exists');
  for (const t of data.tools) {
    const html = await (await fetch(`${B}/compare/${t.tool}/`)).text();
    assert.equal(html.includes('not measured on a corpus'), t.target == null && !formatShares(data, t).length, `${t.tool} target ${t.target}`);
    for (const f of formatShares(data, t)) assert.ok(html.includes(`${Math.floor(f.share)}% of ${f.language} files identical`), `${t.tool} ${f.language}`);
  }
});

test('the compared tools index shows the format share of a formatter, per language', async () => {
  const data = siteData();
  await page.goto(`${B}/compare/`);
  const row = page.locator('main table tbody tr', { has: page.locator('a[href$="/compare/prettier/"]') });
  const labels = await row.locator('.bar-track').evaluateAll((els) => els.map((e) => e.getAttribute('aria-label')));
  const prettier = formatShares(data, data.tools.find((t) => t.tool === 'prettier'));
  assert.equal(labels.length, prettier.length);
  assert.ok(labels.every((l) => l.startsWith('Format: ') && !l.includes('not measured')), labels.join('\n'));
  assert.ok(labels.some((l) => l.endsWith('to Prettier for JavaScript / TypeScript') && l.includes('95%')), labels.join('\n'));
  const both = page.locator('main table tbody tr', { has: page.locator('a[href$="/compare/ruff/"]') });
  assert.deepEqual(await both.locator('.bar-track').evaluateAll((els) => els.map((e) => e.getAttribute('aria-label').split(':')[0])), ['Lint', 'Format']);
  assert.equal(await page.locator('main table tbody tr', { has: page.locator('a[href$="/compare/dotnet-format-editorconfig/"]') }).locator('.bar.none').count(), 1);
});

test('a language page lists its own presets and none of another language', async () => {
  const data = siteData();
  const own = (l) => l.presets;
  for (const l of data.languages) {
    for (const p of own(l)) {
      const folder = p.split('/')[0];
      assert.ok(l.ids.includes(folder)|| folder === 'html', `${l.slug} lists ${p}`);
    }
  }
  await page.goto(`${B}/languages/php/`);
  const presets = await page.locator('main a[href*="/reference/presets/#"]').allInnerTexts();
  assert.deepEqual(presets, ['php/phpstan']);
  await page.goto(`${B}/languages/js/`);
  const js = await page.locator('main a[href*="/reference/presets/#"]').allInnerTexts();
  assert.ok(js.includes('js/oxlint') && js.every((p) => p.startsWith('js/')), js.join(' '));
  await page.goto(`${B}/languages/vue/`); // Vue / Svelte use the presets of the html crate
  assert.ok((await page.locator('main a[href*="/reference/presets/#"]').allInnerTexts()).includes('html/eslint-plugin-vue-recommended'));
});

test('tool and language pages have the header badges', async () => {
  await page.goto(`${B}/compare/hadolint/`);
  assert.equal(await page.locator('.page-head .badge.lang a[href$="/languages/dockerfile/"]').count(), 1);
  assert.equal(await page.locator('.page-head .badge', { hasText: /^version \d/ }).count(), 1);
  assert.equal(await page.locator('.page-head .badge', { hasText: /^\d+% of its findings also reported$/ }).count(), 1);
  for (const w of ['covered', 'partial', 'planned', 'deferred']) assert.equal(await page.locator('.page-head .badge', { hasText: new RegExp(`^\\d+ ${w}$`) }).count(), 1, w);
  assert.equal(await page.getByText(/^Languages:/).count(), 0);
  await page.goto(`${B}/languages/dockerfile/`);
  assert.equal(await page.locator('.page-head .badge.covered', { hasText: 'Ready' }).count(), 1);
  assert.equal(await page.getByText(/^Status:/).count(), 0);
  await page.goto(`${B}/languages/csharp/`); // the support.yaml `requires` sentence
  assert.equal(await page.locator('main a[href*="dotnet.microsoft.com/download/dotnet/10.0"]').count(), 1);
  await page.goto(`${B}/compare/`);
  assert.ok(await page.locator('main table .badge.lang a').count() > 5);
});

test('getting started: install commands equal the README, tabs and steps exist', async () => {
  const readme = readFileSync(new URL('../../README.md', import.meta.url), 'utf8');
  const install = readme.slice(readme.indexOf('## Install'), readme.indexOf('## Why'));
  const commands = [...install.matchAll(/^\| (?:npm|PyPI|Homebrew|Scoop) \| (.*) \|$/gm)].flatMap((m) => [...m[1].matchAll(/`([^`]+)`/g)].map((c) => c[1]));
  assert.ok(commands.length >= 5);
  const shown = channels.map((c) => `${c.command ?? ''} ${c.text ?? ''}`).join('\n');
  for (const c of commands) assert.ok(shown.includes(c), `README command missing on the page: ${c}`);
  for (const c of channels.filter((x) => x.command)) assert.ok(commands.includes(c.command), `the page has a command the README does not: ${c.command}`);
  await page.goto(`${B}/getting-started/`);
  assert.deepEqual(await page.locator('[role=tab]').allInnerTexts(), ['npm', 'PyPI', 'Homebrew', 'Scoop', 'Binary']);
  assert.equal(await page.locator('main h2').count(), 7);
  const twin = await (await fetch(`${B}/getting-started.md`)).text();
  assert.ok(twin.includes('npm i -D @sudsylint/sudsy') && !twin.includes('<TabItem'));
});

test('progress bars: width equals the number, tick at 85, not-measured state, labels, every language', async () => {
  const data = JSON.parse(readFileSync(new URL('../../docs/ai/parity/site.json', import.meta.url), 'utf8'));
  const barsOf = (scope) => page.locator(`${scope} .bar`).evaluateAll((els) => els.map((e) => ({
    cls: e.className, width: e.querySelector('.bar-fill').style.width, num: e.querySelector('.bar-num').textContent,
    label: e.querySelector('[role=img]').getAttribute('aria-label'), tick: getComputedStyle(e.querySelector('.bar-tick')).left, track: e.querySelector('.bar-track').getBoundingClientRect().width,
  })));
  await page.goto(`${B}/languages/`);
  assert.equal(await page.locator('.lang-row').count(), data.languages.length);
  assert.deepEqual(await page.locator('.lang-head a').allInnerTexts(), data.languages.map((l) => l.name));
  const ready = data.languages.filter((l) => /Ready$/.test(l.status)).length;
  const count = (st) => data.languages.filter((l) => l.status === st).length;
  assert.ok((await page.locator('main').innerText()).includes(`${ready} of ${data.languages.length} languages are ready, ${count('In progress')} ${count('In progress') === 1 ? 'is' : 'are'} in progress and ${count('not measured')} ${count('not measured') === 1 ? 'is' : 'are'} not measured yet`));
  assert.ok(!(await page.locator('main').innerText()).includes('every bar at least'));
  const all = await barsOf('main');
  const measured = data.languages.flatMap((l) => [...l.format, ...l.lint]);
  assert.equal(all.length, measured.length);
  for (const b of all) {
    if (b.cls.includes('none')) { assert.equal(b.num, 'not measured'); assert.ok(b.label.includes('not measured')); continue; }
    assert.equal(b.width, b.num, 'fill equals the number');
    assert.ok(b.label.includes(b.num), b.label);
    assert.equal(b.cls.includes('ok'), parseInt(b.num) >= 85);
    assert.ok(Math.abs(parseFloat(b.tick) - 0.85 * b.track) < 2, 'tick at 85%');
  }
  assert.ok(all.some((b) => b.cls.includes('none')), 'a not-measured bar exists');
  assert.ok(all.some((b) => b.cls.includes('primary')) && all.some((b) => !b.cls.includes('primary')));
  assert.ok((await barsOf('main'))[0].label.startsWith('Format: 95% of files identical to Prettier'));
  await page.goto(`${B}/languages/dockerfile/`); // a leg without a reference tool: its text, no bar
  assert.equal(await page.locator('.leg-empty').count(), 1);
  await page.goto(`${B}/compare/`);
  const tools = await barsOf('main table');
  assert.equal(await page.locator('main table tbody tr').count(), data.tools.length);
  const unmeasured = data.tools.filter((t) => t.target == null && !formatShares(data, t).length);
  assert.equal(tools.filter((b) => b.cls.includes('none')).length, unmeasured.length);
  assert.ok(unmeasured.length > 0 && unmeasured.length < data.tools.length / 3, 'a few tools only are not measured');
});

test('no horizontal overflow on desktop and phone', async () => {
  for (const width of [1200, 360]) {
    await page.setViewportSize({ width, height: 900 });
    for (const path of ['rules/', 'rules/sorted-keys/', 'rules/max-line-length/', 'compare/hadolint/', 'compare/', 'languages/', 'languages/dockerfile/', 'languages/js/', 'measured/', '']) {
      await page.goto(`${B}/${path}`);
      if (path === 'rules/') await hits();
      await noOverflow(`${width}px ${path}`);
    }
    if (width === 360) { // the filters collapse into one panel
      await page.goto(`${B}/rules/`);
      await hits();
      assert.equal(await page.locator('.rs-filters').isVisible(), false);
      await page.locator('.rs-toggle').click();
      assert.equal(await page.locator('.rs-toggle').getAttribute('aria-expanded'), 'true');
      assert.equal(await page.locator('.rs-filters').isVisible(), true);
      await noOverflow('360px filters open');
    }
  }
  await page.setViewportSize({ width: 1280, height: 720 });
});

test('the landing page has the new tagline', async () => {
  await page.goto(`${B}/`);
  assert.ok(await page.getByText('Fully featured polyglot formatter and linter for 20+ languages. Extremely configurable, plugin-extendable.').count() >= 1);
});

test('the header search finds rules', async () => {
  await page.goto(`${B}/getting-started/`);
  await page.locator('button[data-open-modal]').first().click();
  await page.locator('dialog[open] input').fill('comment-space');
  await page.locator('dialog[open] a[href*="/rules/comment-space/"]').first().waitFor();
});

test('header logo and favicon load', async () => {
  await page.goto(`${B}/getting-started/`);
  const logo = page.locator('header img').first();
  await logo.waitFor();
  assert.ok(await logo.evaluate((i) => i.complete && i.naturalWidth > 0));
  const href = await page.locator('link[rel~=icon]').first().getAttribute('href');
  assert.ok((await fetch(new URL(href, page.url()))).ok);
});

test('languages: the index follows the data order, every language links to its anchor of the measured page', async () => {
  const { languages } = JSON.parse(readFileSync('../docs/ai/parity/site.json', 'utf8'));
  await page.goto(`${B}/languages/`);
  assert.deepEqual(await page.locator('.lang-row .lang-head > a').allTextContents(), languages.map((l) => l.name));
  // Ready first, then the others; each group by popularity, most popular first.
  const ready = languages.map((l) => /Ready$/.test(l.status));
  assert.ok(ready.every((r, i) => i === 0 || r <= ready[i - 1]), 'ready languages come first');
  for (const group of [true, false]) {
    const p = languages.filter((l, i) => ready[i] === group).map((l) => l.popularity);
    assert.deepEqual(p, [...p].sort((a, b) => b - a), `popularity order of ready=${group}`);
  }
  const hrefs = await page.locator('.lang-how').evaluateAll((a) => a.map((x) => x.getAttribute('href')));
  assert.deepEqual(hrefs, languages.map((l) => `/sudsy/measured/#lang-${l.slug}`));
  await page.goto(`${B}/measured/`);
  for (const l of languages) assert.equal(await page.locator(`[id="lang-${l.slug}"]`).count(), 1, `anchor ${l.slug}`);
  // The page is docs/languages.md: the section of each language is there, with the generated numbers.
  const headings = await page.locator('h2').allTextContents();
  for (const l of languages) assert.ok(headings.some((h) => h.replace(/\u200b/g, '').trim() === l.section), l.section);
  assert.ok(await page.getByText('Measured', { exact: false }).count() >= languages.length);
  // Links of the page work: site links answer, anchors exist.
  for (const href of await page.locator('main a[href^="/sudsy/"]').evaluateAll((a) => [...new Set(a.map((x) => x.getAttribute('href')))])) {
    assert.ok((await fetch(`http://localhost:${PORT}${href.split('#')[0]}`)).ok, href);
  }
  await page.goto(`${B}/languages/php/`);
  const intro = await page.locator('.sl-markdown-content').innerText();
  assert.match(intro, /default rule set/);
  assert.match(intro, /except PHP-CS-Fixer: measured with the @PER-CS rule set/);
  await page.goto(`${B}/languages/python/`);
  assert.doesNotMatch(await page.locator('.sl-markdown-content').innerText(), /except /);
});

const sessions = JSON.parse(readFileSync(new URL('../src/data/sessions.json', import.meta.url), 'utf8'));

test('landing page: every section, one h1, the numbers come from the data', async () => {
  const c = counts();
  const data = siteData();
  await page.goto(`${B}/`);
  for (const id of ['hero-title', 'sessions-title', 'one-title', 'langs-title', 'speed-title', 'why-title', 'last-title']) assert.equal(await page.locator(`#${id}`).count(), 1, id);
  assert.equal(await page.locator('h1:visible').count(), 1);
  const text = await page.locator('.landing').innerText();
  assert.ok(text.includes(`Ready for ${c.ready} languages, early support for ${c.progress} more.`));
  // the claim is computed from the speed rows and links to the full list, where the losers are marked
  const claim = lintClaim();
  assert.ok(claim.total > 0 && text.includes(claimText(claim).replace(/^Faster/, 'faster')));
  const sub = page.locator('#speed-title').locator('xpath=../p');
  assert.ok((await sub.innerText()).startsWith(`Sudsy is ${claimText(claim).replace(/^Faster/, 'faster')}.`) && !text.includes('Where Sudsy is slower'));
  assert.equal(await sub.locator('a').getAttribute('href'), '/sudsy/speed/');
  assert.equal(await page.locator('.speed ~ .more a', { hasText: 'Every language' }).getAttribute('href'), '/sudsy/speed/');
  assert.ok(text.includes(`${c.ready} languages ready, ${c.progress} in progress`));
  assert.ok(text.includes(`${c.rules} rules with their own pages`) && text.includes(`${c.presets} presets`) && text.includes(`${c.safe} rules carry a fix marked safe`));
  assert.ok(text.includes('Early release (v0.0.x): config may change in minor versions before 1.0.'));
  // the language grid: one tile per language of the support table, each linked, with its status
  const tiles = page.locator('.tile');
  const shown = data.languages.slice(0, 12); // the most popular, in the order of the data; the link leads to the rest
  assert.equal(await tiles.count(), shown.length);
  assert.deepEqual(await tiles.locator('.tile-head a').allInnerTexts(), shown.map((l) => l.name));
  assert.deepEqual(await tiles.locator('.tile-head a').evaluateAll((as) => as.map((a) => new URL(a.href).pathname)), shown.map((l) => `/sudsy/languages/${l.slug}/`));
  assert.equal(await tiles.locator('.badge.covered').count(), shown.filter((l) => /Ready$/.test(l.status)).length);
  assert.equal(await page.locator('.landing .more a', { hasText: `All ${data.languages.length} languages` }).getAttribute('href'), '/sudsy/languages/');
  const first = await tiles.first().locator('.bar-track').evaluateAll((els) => els.map((e) => e.getAttribute('aria-label')));
  const f0 = shown[0].format[0];
  assert.ok(first[0].startsWith(`Format: ${Math.floor(f0.share)}% of files identical to ${f0.name}`) && first.length === 2, first.join('|'));
  // a tool measured with another rule set says so on its tile
  for (const l of shown) for (const x of [l.format[0], l.lint[0]].filter((x) => x?.note)) assert.ok(await page.locator('.tile', { hasText: l.name }).locator('.tile-note', { hasText: x.note }).count() === 1, `${l.name}: ${x.note}`);
  // speed: the generated rows
  const rows = speedRows();
  assert.equal(await page.locator('.speed-card').count(), rows.length);
  for (const r of rows) assert.ok(await page.locator('.speed-card', { hasText: r.verdict }).locator(`text=${r.sudsyText}`).count() >= 1, r.tool);
  assert.ok(await page.locator('.speed-card .fill').evaluateAll((els) => els.every((e) => parseFloat(e.style.width) > 0 && parseFloat(e.style.width) <= 100)));
  // reasons: at most six, each linked
  const cards = page.locator('.card');
  assert.ok(await cards.count() <= 6 && await cards.count() >= 4);
  assert.equal(await cards.locator('a').count(), await cards.count());
});

test('speed page: every measured row, the languages that lose are marked, the claim counts match', async () => {
  const data = siteData();
  const claim = lintClaim();
  await page.goto(`${B}/speed/`);
  assert.equal(await page.locator('main tbody tr').count(), data.languages.reduce((n, l) => n + l.speed.length, 0));
  const text = await page.locator('main').innerText();
  assert.ok(text.includes(`${claimText(claim)}: ${claim.faster.length} of ${claim.total}`));
  for (const r of claim.slower) assert.ok(text.includes(`${r.language} (${r.tool}, ${r.verdict})`), r.language);
  assert.equal(await page.locator('main tbody .badge.partial').count(), data.languages.flatMap((l) => l.speed).filter((s) => s[3].includes('slower')).length);
  assert.ok(claim.slower.length > 0 && claim.left.every((r) => r.reason));
});

test('landing page: the CI step and the config are the README\'s and the demo\'s', async () => {
  await page.goto(`${B}/`);
  assert.equal((await page.locator('#before-after code').textContent()).trimEnd(), beforeAfter());
  assert.equal((await page.locator('#config code').textContent()).trimEnd(), readFileSync(new URL('../demo/sudsy.yaml', import.meta.url), 'utf8').trimEnd());
  assert.ok(await page.locator('#before-after .t-del').count() >= 2 && await page.locator('#before-after .t-add').count() === 1);
});

test('landing page: install tabs switch by click and keyboard, copy works; commands equal the README', async () => {
  await page.goto(`${B}/`);
  const readme = readFileSync(new URL('../../README.md', import.meta.url), 'utf8');
  for (const id of ['hero', 'last']) {
    const tabs = page.locator(`#${id}-tab-0`).locator('xpath=..');
    assert.deepEqual(await tabs.locator('[role=tab]').allInnerTexts(), channels.map((c) => c.label));
    await page.locator(`#${id}-tab-1`).click();
    assert.equal(await page.locator(`#${id}-tab-1`).getAttribute('aria-selected'), 'true');
    assert.equal(await page.locator(`#${id}-panel-0`).isVisible(), false);
    assert.ok((await page.locator(`#${id}-panel-1`).innerText()).includes('pip install sudsy'));
    await page.locator(`#${id}-tab-1`).press('ArrowRight');
    assert.equal(await page.locator(`#${id}-tab-2`).getAttribute('aria-selected'), 'true');
    assert.ok(await page.locator(`#${id}-panel-2`).isVisible());
    await page.locator(`#${id}-tab-2`).press('End');
    assert.ok((await page.locator(`#${id}-panel-4`).innerText()).includes('GitHub releases'));
  }
  for (const c of channels.filter((x) => x.command)) assert.ok(readme.includes(`\`${c.command}\``) || readme.includes(c.command), c.command);
  await page.locator('#hero-tab-0').click();
  await page.evaluate(() => { window.__copied = null; Object.defineProperty(navigator, 'clipboard', { value: { writeText: async (t) => { window.__copied = t; } }, configurable: true }); });
  await page.locator('#hero-panel-0 .copy').click();
  await page.waitForFunction(() => window.__copied !== null);
  assert.equal(await page.evaluate(() => window.__copied), 'npm i -D @sudsylint/sudsy');
  assert.equal(await page.locator('#hero-panel-0 .copy').innerText(), 'Copied');
});

test('landing page: the terminal shows what the CLI printed and its tabs switch', async () => {
  await page.goto(`${B}/`);
  assert.deepEqual(await page.locator('.terminal [role=tab]').allInnerTexts(), sessions.map((s) => s.label));
  for (const s of sessions) {
    await page.locator(`#term-tab-${s.id}`).click();
    const shown = await page.locator(`#term-panel-${s.id}`).innerText();
    for (const st of s.steps) assert.ok(shown.includes(`$ ${st.command}`) && shown.includes(st.output.split('\n')[0]) && shown.includes(st.output.split('\n').at(-1)), `${s.id}: ${st.command}`);
  }
  assert.ok(sessions[0].steps[0].output.includes('error[no-debugger]') && sessions[2].steps[1].output.includes('@@'));
  assert.ok(await page.locator('#term-panel-format .t-add').count() > 0 && await page.locator('#term-panel-format .t-del').count() > 0);
});

test('landing page: usable without JavaScript, with reduced motion, and 360 px wide', async () => {
  const plain = await browser.newContext({ javaScriptEnabled: false });
  const p = await plain.newPage();
  await p.goto(`${B}/`);
  assert.equal(await p.locator('.hero [role=tabpanel]:visible').count(), 1);
  assert.ok((await p.locator('.hero [role=tabpanel]:visible').innerText()).includes('npm i -D @sudsylint/sudsy'));
  assert.equal(await p.locator('.hero .tab-list').isVisible(), false);
  assert.equal(await p.locator('.hero .copy').first().isVisible(), false);
  assert.equal(await p.locator('.terminal [role=tabpanel]:visible').count(), 1);
  assert.equal(await p.locator('.tile').count(), 12);
  await plain.close();
  const calm = await browser.newContext({ reducedMotion: 'reduce' });
  const q = await calm.newPage();
  await q.goto(`${B}/`);
  assert.equal(await q.locator('.button.primary').first().evaluate((e) => getComputedStyle(e).transitionDuration), '0s');
  assert.equal(await q.locator('.bar-fill').first().evaluate((e) => getComputedStyle(e).transitionDuration), '0s');
  await calm.close();
  await page.setViewportSize({ width: 360, height: 800 });
  await page.goto(`${B}/`);
  await noOverflow('360px landing');
  assert.ok(await page.locator('.hero .install').evaluate((e) => e.getBoundingClientRect().right <= 360));
  await page.setViewportSize({ width: 1280, height: 720 });
});

test('landing page and getting started pass axe (WCAG A and AA), light and dark', async () => {
  const axe = readFileSync(new URL('../node_modules/axe-core/axe.min.js', import.meta.url), 'utf8');
  for (const path of ['', 'getting-started/', 'compare/', 'languages/php/']) {
    for (const scheme of ['light', 'dark']) {
      await page.emulateMedia({ colorScheme: scheme });
      await page.goto(`${B}/${path}`);
      await page.evaluate(axe);
      const bad = await page.evaluate(async () => (await window.axe.run(document, { runOnly: ['wcag2a', 'wcag2aa'] })).violations.map((v) => `${v.id}: ${v.nodes.slice(0, 3).map((n) => n.target.join(' ')).join(', ')}`));
      assert.deepEqual(bad, [], `${path || 'landing'} ${scheme}`);
    }
  }
  await page.emulateMedia({ colorScheme: 'light' });
});

test('landing page weight stays small', async () => {
  const ctx = await browser.newContext();
  const p = await ctx.newPage();
  let bytes = 0;
  p.on('response', async (r) => { try { bytes += (await r.body()).length; } catch {} });
  await p.goto(`${B}/`, { waitUntil: 'networkidle' });
  await ctx.close();
  assert.ok(bytes < 500 * 1024, `${Math.round(bytes / 1024)} KB`);
});

test('no console or network errors', () => assert.deepEqual(problems, []));
