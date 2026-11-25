// The terminal sessions of the landing page: the CLI run on the small repository in site/demo/, its output
// written to src/data/sessions.json (checked in, so the site build needs no CLI). `node scripts/sessions.mjs`
// rewrites the file; `--check` (part of `scripts/site.sh test`) fails when the file differs from a fresh run.
// The CLI is `$SUDSY_BIN`, else `debug/sudsy` of `$CARGO_TARGET_DIR` (`cargo build -p cli`).
import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const here = (p) => fileURLToPath(new URL(p, import.meta.url));
const target = process.env.CARGO_TARGET_DIR ?? here('../../target');
const bin = process.env.SUDSY_BIN ?? `${target}/debug/sudsy`;
const out = here('../src/data/sessions.json');
if (!existsSync(bin)) {
  console.error(`sessions: no CLI at ${bin}; build it (cargo build -p cli) or set SUDSY_BIN`);
  process.exit(2);
}

const work = `${target}/site-demo`;
const fresh = (name) => {
  const dir = `${work}/${name}`;
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(work, { recursive: true });
  cpSync(here('../demo'), dir, { recursive: true });
  return dir;
};
const run = (cwd, argv) => {
  const [cmd, ...args] = argv;
  const r = spawnSync(cmd === 'sudsy' ? bin : cmd, args, { cwd, encoding: 'utf8', env: { ...process.env, NO_COLOR: '1' } });
  return `${r.stdout}${r.stderr}`.replace(/[ \t]+$/gm, '').trimEnd();
};
const step = (cwd, argv) => ({ command: argv.join(' '), output: run(cwd, argv) });

const check = fresh('check');
const fix = fresh('fix');
const format = fresh('format');
const git = (...a) => ['git', '-c', 'user.name=demo', '-c', 'user.email=demo@example.com', ...a];
run(format, ['git', 'init', '-q']);
run(format, git('add', '.'));
run(format, git('commit', '-q', '-m', 'demo'));

const sessions = [
  { id: 'check', label: 'check', steps: [step(check, ['sudsy', 'check'])] },
  { id: 'fix', label: 'fix', steps: [step(fix, ['sudsy', 'fix'])] },
  { id: 'format', label: 'format', steps: [step(format, ['sudsy', 'format']), step(format, ['git', 'diff'])] },
];
const text = JSON.stringify(sessions, null, 2) + '\n';

if (process.argv.includes('--check')) {
  if (readFileSync(out, 'utf8') !== text) {
    console.error('sessions: src/data/sessions.json is stale; run `node scripts/sessions.mjs` in site/ and commit it');
    process.exit(1);
  }
} else {
  writeFileSync(out, text, { flag: 'w' });
}
