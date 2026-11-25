// Shared by astro.config.mjs, scripts/generate.mjs and the landing components. Paths are relative to the working
// directory (site/): the components run from a bundle, where import.meta.url no longer points into the repository.
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

export const site = 'https://bingsubandit.github.io';
export const base = '/sudsy';
export const rulesPath = process.env.SUDSY_RULES_JSON ?? resolve(process.cwd(), '../docs/ai/rules/rules.json');
export const rules = () => JSON.parse(readFileSync(rulesPath, 'utf8')).rules;
// Languages (support table, speed rows, presets) and compared tools (parity catalogs): cargo xtask codegen.
export const siteDataPath = resolve(process.cwd(), '../docs/ai/parity/site.json');
export const siteData = () => JSON.parse(readFileSync(siteDataPath, 'utf8'));
