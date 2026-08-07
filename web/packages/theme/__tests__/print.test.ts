import { describe, it, expect } from 'vitest';
import { printCss } from '../src/print.js';
import { nigelTheme } from '../src/themes/nigel.js';

const print = printCss.cssText;
const composed = nigelTheme.cssText;

/**
 * A print stylesheet cannot be proved by a screenshot in CI, so these assert
 * the rules that carry the behaviour — the ones whose absence would silently
 * put a sidebar, a dark background or a headerless second page on the paper.
 * The manual browser pass is recorded in the task notes and in web/README.md.
 */
describe('printCss', () => {
  it('sets a page margin', () => {
    expect(print).toMatch(/@page\s*{[^}]*margin:\s*1\.5cm/);
  });

  it('scopes everything else to @media print', () => {
    expect(print).toContain('@media print');
  });

  it.each([
    ['--wa-color-bg', '#ffffff'],
    ['--wa-color-surface', '#ffffff'],
    ['--wa-color-text', '#000000'],
    ['--nc-color-income', '#000000'],
    ['--nc-color-expense', '#000000'],
  ])('repaints %s for paper', (token, value) => {
    // Redefining the token at :root is what reaches inside every shadow root;
    // no print rule can select a component's internals directly.
    expect(print).toMatch(new RegExp(`${token}:\\s*${value}`));
  });

  it('drops the brand gradient and the shadows', () => {
    expect(print).toMatch(/--nc-grad-brand:\s*none/);
    expect(print).toMatch(/--wa-shadow-m:\s*none/);
  });

  it.each(['sidebar', 'header', 'banner'])('hides the shell %s part', (part) => {
    expect(print).toContain(`wc-app-shell::part(${part})`);
  });

  it('gives the page over to the content part', () => {
    expect(print).toMatch(/wc-app-shell::part\(content\)\s*{[^}]*padding:\s*0/);
  });

  it.each(['wc-nav-sidebar', 'wc-toast', 'wc-export-links', 'wc-period-nav'])(
    'hides %s',
    (tag) => {
      expect(print).toContain(tag);
    },
  );

  it('repeats table headings across page breaks', () => {
    expect(print).toMatch(/thead\s*{[^}]*display:\s*table-header-group/);
  });

  it('keeps rows and panels from splitting across pages', () => {
    expect(print).toMatch(/break-inside:\s*avoid/);
  });

  it('ships inside the composed theme, after the dark overrides', () => {
    expect(composed).toContain(print);
    // Last wins in a flat cascade, and dark mode must not survive onto paper.
    expect(composed.indexOf('@media print')).toBeGreaterThan(
      composed.indexOf('--wa-color-bg: #17171d'),
    );
  });
});
