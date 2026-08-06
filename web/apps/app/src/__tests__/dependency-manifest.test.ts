import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * Every `@nigel/*` package imported under src has to be declared in this
 * app's package.json. Undeclared ones resolve today only because npm hoists
 * workspace packages to the root node_modules, and break the moment the
 * install is isolated.
 */
const here = dirname(fileURLToPath(import.meta.url));
const appRoot = resolve(here, '../../');
const srcDir = resolve(here, '../');

function walk(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === 'node_modules' || entry.name === 'dist') continue;
      out.push(...walk(full));
    } else if (entry.name.endsWith('.ts')) {
      out.push(full);
    }
  }
  return out;
}

describe('dependency manifest', () => {
  it('declares every @nigel/* package imported under src', () => {
    const pkg = JSON.parse(readFileSync(join(appRoot, 'package.json'), 'utf8')) as {
      name: string;
      dependencies?: Record<string, string>;
      devDependencies?: Record<string, string>;
    };
    const declared = new Set([
      ...Object.keys(pkg.dependencies ?? {}),
      ...Object.keys(pkg.devDependencies ?? {}),
    ]);

    const referenced = new Set<string>();
    for (const file of walk(srcDir)) {
      for (const match of readFileSync(file, 'utf8').matchAll(/@nigel\/[a-z0-9-]+/g)) {
        if (match[0] !== pkg.name) referenced.add(match[0]);
      }
    }

    const missing = [...referenced].filter((p) => !declared.has(p)).sort();
    expect(missing, `undeclared @nigel deps: ${missing.join(', ')}`).toEqual([]);
  });
});
