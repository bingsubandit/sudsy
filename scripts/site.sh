#!/usr/bin/env bash
# Documentation site (site/). Usage: scripts/site.sh build | dev | preview | test | scale-test [N] | check-links [tool...] | list-bench [PORT] | landing-bench [PORT] | sessions [--check]
#   build        npm ci + generate pages from docs/ai/rules/rules.json + astro build -> site/dist
#   test         build + browser smoke test (site/test/smoke.mjs; Playwright drives installed Edge or Chrome)
#   dev          generate + astro dev server (no search: the index exists only in a build)
#   preview      build, then serve site/dist with search at http://localhost:4321/sudsy/
#   check-links  every upstream rule-url of the parity catalogs answers 200 and has its anchor (network; nightly)
#   list-bench   page weight, full-list time, first filter and search of /rules/ in the build (served on PORT, default 4396)
#   landing-bench  page weight and largest contentful paint of the landing page in the build (served on PORT, default 4397)
#   sessions     rewrite src/data/sessions.json from a run of the CLI on site/demo (--check: fail when it is stale; needs the debug CLI of the slot or SUDSY_BIN)
#   scale-test   build with N (default 10000) synthetic rules from a copy of rules.json in the build
#                slot ($CARGO_TARGET_DIR/site-scale); prints build time and output/search index sizes
set -euo pipefail
cd "$(dirname "$0")/../site"
case "${1:-build}" in
  build) npm ci && npm run build ;;
  dev) npm ci && npm run dev ;;
  preview) npm ci && npm run build && npm run preview ;;
  check-links) shift; node scripts/check-urls.mjs "$@" ;;
  list-bench)
    port=${2:-4396}
    npx astro preview --port "$port" >/dev/null; sleep 2
    node scripts/list-bench.mjs "$port" || { npx astro preview stop >/dev/null; exit 1; }
    npx astro preview stop >/dev/null ;;
  landing-bench)
    port=${2:-4397}
    npx astro preview --port "$port" >/dev/null; sleep 2
    node scripts/landing-bench.mjs "$port" || { npx astro preview stop >/dev/null; exit 1; }
    npx astro preview stop >/dev/null ;;
  sessions) shift; node scripts/sessions.mjs "$@" ;;
  test) npm ci && node scripts/sessions.mjs --check && npm run build && node --test test/smoke.mjs ;;
  scale-test)
    n=${2:-10000}
    dir=${CARGO_TARGET_DIR:-../target}/site-scale
    rm -rf "$dir" && mkdir -p "$dir"
    node -e '
      const [src, dst, n] = process.argv.slice(1);
      const real = JSON.parse(require("fs").readFileSync(src, "utf8")).rules;
      const rules = Array.from({ length: +n }, (_, i) => {
        const r = structuredClone(real[i % real.length]);
        r.id += `-${i}`;
        r.upstream.forEach((u) => (u.id += `${i}`));
        return r;
      });
      require("fs").writeFileSync(dst, JSON.stringify({ rules }));
    ' ../docs/ai/rules/rules.json "$dir/rules.json" "$n"
    npm ci
    export SUDSY_RULES_JSON=$(cd "$dir" && pwd)/rules.json SITE_OUT=$(cd "$dir" && pwd)/dist
    start=$(date +%s)
    npm run build >"$dir/build.log" 2>&1 || { tail -30 "$dir/build.log"; exit 1; }
    end=$(date +%s)
    echo "rules: $n  build: $((end - start)) s"
    echo "pages: $(find "$SITE_OUT" -name index.html | wc -l)"
    echo "output: $(du -sk --apparent-size "$SITE_OUT" | cut -f1) KB, search index: $(du -sk --apparent-size "$SITE_OUT/pagefind" | cut -f1) KB"
    env -u SUDSY_RULES_JSON npm run generate >/dev/null  # leave the real pages in place
    ;;
  *) echo "usage: scripts/site.sh build | dev | preview | test | scale-test [N] | check-links [tool...] | list-bench [PORT]" >&2; exit 2 ;;
esac
