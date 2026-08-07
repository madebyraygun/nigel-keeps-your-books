import { describe, it, expect } from 'vitest';
import { nigelTheme } from '../src/themes/nigel.js';

/**
 * The brand palette is pastel, which is exactly the failure mode this guards:
 * a pastel foreground on a white surface looks fine in a mockup and is
 * unreadable in use. Every foreground/background pairing the UI actually
 * renders is held to WCAG AA (4.5:1) in both modes.
 */

const AA_NORMAL = 4.5;

function channel(hex: string): [number, number, number] {
  const v = hex.replace('#', '');
  return [
    parseInt(v.slice(0, 2), 16),
    parseInt(v.slice(2, 4), 16),
    parseInt(v.slice(4, 6), 16),
  ];
}

function relativeLuminance(hex: string): number {
  const srgb = channel(hex).map((c) => {
    const s = c / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * srgb[0] + 0.7152 * srgb[1] + 0.0722 * srgb[2];
}

function contrast(a: string, b: string): number {
  const la = relativeLuminance(a);
  const lb = relativeLuminance(b);
  const [hi, lo] = la > lb ? [la, lb] : [lb, la];
  return (hi + 0.05) / (lo + 0.05);
}

/**
 * Pull a token's value out of the composed sheet. `occurrence` selects between
 * the light declaration (0) and the dark ones that follow it.
 */
function token(name: string, occurrence: number): string {
  const matches = [
    ...nigelTheme.cssText.matchAll(
      new RegExp(`${name}:\\s*(#[0-9a-fA-F]{6})`, 'g'),
    ),
  ].map((m) => m[1].toLowerCase());
  const value = matches[occurrence];
  expect(value, `${name} occurrence ${occurrence} not found`).toBeDefined();
  return value;
}

const light = (name: string) => token(name, 0);
const dark = (name: string) => token(name, 1);

describe('contrast helper', () => {
  it('computes the reference extremes', () => {
    expect(contrast('#000000', '#ffffff')).toBeCloseTo(21, 1);
    expect(contrast('#ffffff', '#ffffff')).toBeCloseTo(1, 5);
  });
});

describe.each([
  ['light', light],
  ['dark', dark],
])('%s mode meets WCAG AA', (_mode, t) => {
  it.each([
    ['text on bg', '--wa-color-text', '--wa-color-bg'],
    ['text on surface', '--wa-color-text', '--wa-color-surface'],
    ['text on surface-alt', '--wa-color-text', '--wa-color-surface-alt'],
    ['muted on bg', '--wa-color-muted', '--wa-color-bg'],
    ['muted on surface', '--wa-color-muted', '--wa-color-surface'],
    ['on-brand on brand', '--wa-color-on-brand', '--wa-color-brand'],
    ['brand on bg', '--wa-color-brand', '--wa-color-bg'],
    ['danger on surface', '--wa-color-danger', '--wa-color-surface'],
    ['success on surface', '--wa-color-success', '--wa-color-surface'],
    ['warning on surface', '--wa-color-warning', '--wa-color-surface'],
    ['info on surface', '--wa-color-info', '--wa-color-surface'],
    ['income on bg', '--nc-color-income', '--wa-color-bg'],
    ['income on surface', '--nc-color-income', '--wa-color-surface'],
    ['expense on bg', '--nc-color-expense', '--wa-color-bg'],
    ['expense on surface', '--nc-color-expense', '--wa-color-surface'],
    ['flagged on surface', '--nc-color-flagged', '--wa-color-surface'],
    ['text on selected row', '--wa-color-text', '--nc-color-selected-bg'],
  ])('%s', (_label, fg, bg) => {
    expect(contrast(t(fg), t(bg))).toBeGreaterThanOrEqual(AA_NORMAL);
  });
});
