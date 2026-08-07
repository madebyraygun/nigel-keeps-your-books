import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { NIGEL_PALETTE } from '../src/tokens/gradient.js';

/**
 * The web palette is meant to *be* the TUI palette, not merely resemble it.
 * This reads the Rust source and fails if the two drift apart, which keeps the
 * derivation load bearing instead of decorative.
 */
const here = dirname(fileURLToPath(import.meta.url));
const effectsRs = resolve(here, '../../../../src/effects.rs');

function paletteFromRust(): string[] {
  const source = readFileSync(effectsRs, 'utf8');
  const start = source.indexOf('pub const GRADIENT');
  expect(start, 'GRADIENT const not found in src/effects.rs').toBeGreaterThan(-1);
  const end = source.indexOf('];', start);
  const block = source.slice(start, end);
  const hexes = [...block.matchAll(/#([0-9a-fA-F]{6})/g)].map(
    (m) => `#${m[1].toLowerCase()}`,
  );
  // The Rust array wraps back to the first stop so the interpolation closes;
  // the CSS ramp has no such need.
  return [...new Set(hexes)];
}

describe('palette parity with src/effects.rs', () => {
  it('reads a non-empty palette out of the Rust source', () => {
    expect(paletteFromRust().length).toBeGreaterThan(0);
  });

  it('matches NIGEL_PALETTE exactly, in order', () => {
    expect(paletteFromRust()).toEqual([...NIGEL_PALETTE]);
  });
});
