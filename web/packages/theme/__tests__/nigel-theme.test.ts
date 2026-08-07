import { describe, it, expect } from 'vitest';
import { nigelTheme } from '../src/themes/nigel.js';

const text = nigelTheme.cssText;

describe('nigelTheme', () => {
  it('exposes a Lit CSSResult with the composed token sheet', () => {
    expect(nigelTheme).toBeDefined();
    expect(typeof text).toBe('string');
    expect(text.length).toBeGreaterThan(0);
  });

  it.each([
    '--wa-color-bg',
    '--wa-color-surface',
    '--wa-color-surface-alt',
    '--wa-color-border',
    '--wa-color-border-soft',
    '--wa-color-text',
    '--wa-color-muted',
    '--wa-color-brand',
    '--wa-color-brand-hover',
    '--wa-color-on-brand',
    '--wa-color-focus',
    '--wa-color-danger',
    '--wa-color-success',
    '--wa-color-warning',
    '--wa-color-info',
    '--wa-font-family-sans',
    '--wa-font-family-mono',
    '--wa-font-size-base',
    '--wa-line-height',
    '--wa-space-m',
    '--wa-radius-md',
    '--wa-shadow-sm',
  ])('defines the Web Awesome token %s', (token) => {
    expect(text).toContain(`${token}:`);
  });

  it.each([
    '--nc-color-income',
    '--nc-color-expense',
    '--nc-color-flagged',
    '--nc-color-selected-bg',
    '--nc-grad-brand',
    '--nc-grad-brand-soft',
    '--nc-font-money',
    '--nc-icon-size',
    '--nc-sidebar-width',
    '--nc-sidebar-collapsed-width',
    '--nc-header-height',
    '--nc-transition-fast',
    '--nc-transition-base',
  ])('defines the nigel token %s', (token) => {
    expect(text).toContain(`${token}:`);
  });

  it('supports system dark mode and both explicit overrides', () => {
    expect(text).toMatch(/prefers-color-scheme:\s*dark/);
    expect(text).toContain('.dark-mode');
    expect(text).toContain('.light-mode');
  });

  it('honours a reduced-motion preference', () => {
    expect(text).toMatch(/prefers-reduced-motion:\s*reduce/);
  });

  it('carries the global wa-* shadow-part overrides', () => {
    expect(text).toContain('wa-button');
    expect(text).toContain('wa-dialog');
    expect(text).toContain('::part(base)');
    expect(text).toContain('::part(label)');
    expect(text).toContain('::part(form-control-label)');
  });

  it('orders light tokens before dark overrides before the part overrides', () => {
    // Specificity alone does not settle this: the dark block and the light
    // block both target :root, so the later one wins. Order is the contract.
    const light = text.indexOf('--wa-color-bg: #fdfcfb');
    const dark = text.indexOf('.dark-mode');
    const parts = text.indexOf('::part(base)');
    expect(light).toBeGreaterThan(-1);
    expect(light).toBeLessThan(dark);
    expect(dark).toBeLessThan(parts);
  });
});
