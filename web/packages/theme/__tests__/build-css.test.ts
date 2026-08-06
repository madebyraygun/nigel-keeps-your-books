import { describe, it, expect, beforeAll } from 'vitest';
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { nigelTheme } from '../src/themes/nigel.js';

const here = dirname(fileURLToPath(import.meta.url));
const pkgRoot = resolve(here, '..');
const cssPath = resolve(pkgRoot, 'dist/css/nigel.css');

describe('build-css', () => {
  beforeAll(() => {
    // The script consumes dist/themes/nigel.js, so compile first. Running the
    // real pipeline is the point: a stylesheet that only exists because a test
    // wrote it proves nothing about `npm run build`.
    execFileSync('npx', ['tsc'], { cwd: pkgRoot, stdio: 'pipe' });
    execFileSync('node', ['scripts/build-css.js'], { cwd: pkgRoot, stdio: 'pipe' });
  }, 120_000);

  it('emits dist/css/nigel.css', () => {
    expect(existsSync(cssPath)).toBe(true);
  });

  it('writes the banner and the full composed sheet', () => {
    const css = readFileSync(cssPath, 'utf8');
    expect(css).toContain('Nigel theme tokens');
    expect(css).toContain(nigelTheme.cssText);
  });
});
