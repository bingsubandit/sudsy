// Landing page of a built site: page weight (decoded bytes by type), requests and the largest contentful paint,
// median of 5 loads, desktop and 360 px. `scripts/site.sh landing-bench [PORT]` serves the build and runs this;
// PORT is the one `astro preview` listens on.
import { chromium } from 'playwright-core';
const B = `http://localhost:${process.argv[2]}/sudsy`;
let browser;
for (const options of [{ channel: 'msedge' }, { channel: 'chrome' }, {}]) {
  try { browser = await chromium.launch(options); break; } catch {}
}
const median = (a) => [...a].sort((x, y) => x - y)[Math.floor(a.length / 2)];
const out = {};
for (const [name, width] of [['desktop', 1280], ['phone', 360]]) {
  const lcps = [];
  let bytes = {}, requests = 0;
  for (let i = 0; i < 5; i++) {
    const ctx = await browser.newContext({ viewport: { width, height: 800 } });
    const page = await ctx.newPage();
    const run = { bytes: {}, requests: 0 };
    page.on('response', async (r) => {
      try {
        const kind = (r.headers()['content-type'] ?? '').split(';')[0].split('/').pop();
        run.bytes[kind] = (run.bytes[kind] ?? 0) + (await r.body()).length;
        run.requests++;
      } catch {}
    });
    await page.addInitScript(() => {
      window.__lcp = 0;
      new PerformanceObserver((l) => { for (const e of l.getEntries()) window.__lcp = e.startTime; }).observe({ type: 'largest-contentful-paint', buffered: true });
    });
    await page.goto(`${B}/`, { waitUntil: 'networkidle' });
    lcps.push(Math.round(await page.evaluate(() => window.__lcp)));
    bytes = run.bytes; requests = run.requests;
    await ctx.close();
  }
  out[name] = { lcpMs: median(lcps), requests, weightKB: Math.round(Object.values(bytes).reduce((a, b) => a + b, 0) / 1024), byTypeKB: Object.fromEntries(Object.entries(bytes).map(([k, v]) => [k, Math.round(v / 1024)])) };
}
console.log(JSON.stringify(out));
await browser.close();
