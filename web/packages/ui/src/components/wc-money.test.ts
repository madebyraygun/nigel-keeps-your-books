import { describe, it, expect, afterEach } from 'vitest';
import './wc-money.js';
import type { WcMoney } from './wc-money.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-money.preview.js';

async function mount(props: Partial<WcMoney> = {}): Promise<WcMoney> {
  const el = document.createElement('wc-money');
  Object.assign(el, { locale: 'en-US', ...props });
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function text(el: WcMoney): string {
  return el.shadowRoot?.querySelector('.amount')?.textContent?.trim() ?? '';
}

function sign(el: WcMoney): string | null {
  return el.shadowRoot?.querySelector('.amount')?.getAttribute('data-sign') ?? null;
}

describe('wc-money', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  // The exact vectors src/fmt.rs asserts for `money()`. The web UI and the TUI
  // print the same strings or the two front ends disagree about the books.
  it.each([
    [1234.56, '$1,234.56'],
    [-500.0, '-$500.00'],
    [0, '$0.00'],
    [1000000.99, '$1,000,000.99'],
    [42.1, '$42.10'],
  ])('formats %d as %s, matching fmt::money', async (amount, expected) => {
    const el = await mount({ amount });
    expect(text(el)).toBe(expected);
  });

  // Rust reads the stored double and settles an exact tie on the even
  // neighbour; Intl left to itself reads the shortest decimal and rounds a tie
  // away from zero, which is a cent's disagreement on a tax worksheet.
  it.each([
    [0.565, '$0.56'],
    [1.005, '$1.00'],
    [8.835, '$8.84'],
    [0.125, '$0.12'],
    [0.375, '$0.38'],
    [617.375, '$617.38'],
    [-0.125, '-$0.12'],
  ])('rounds %d to %s, matching `{:.2}`', async (amount, expected) => {
    const el = await mount({ amount });
    expect(text(el)).toBe(expected);
  });

  it('renders the minus sign rather than relying on color alone', async () => {
    const el = await mount({ amount: -500 });
    expect(text(el)).toContain('-');
  });

  it('marks positive amounts as income', async () => {
    expect(sign(await mount({ amount: 10 }))).toBe('positive');
  });

  it('marks negative amounts as expense', async () => {
    expect(sign(await mount({ amount: -10 }))).toBe('negative');
  });

  it('treats zero as neutral by default', async () => {
    expect(sign(await mount({ amount: 0 }))).toBe('zero');
  });

  it('treats zero as income when zeroNeutral is off', async () => {
    expect(sign(await mount({ amount: 0, zeroNeutral: false }))).toBe('positive');
  });

  it('drops the colour cue in the plain variant', async () => {
    expect(sign(await mount({ amount: -10, variant: 'plain' }))).toBe('plain');
  });

  it('honours a non-USD currency', async () => {
    const el = await mount({ amount: 12.5, currency: 'EUR' });
    expect(text(el)).toContain('12.50');
  });

  it('exposes the formatted text to callers', async () => {
    expect((await mount({ amount: -1 })).formatted).toBe('-$1.00');
  });

  it('uses tabular figures so columns align', async () => {
    const el = await mount({ amount: 1 });
    const styles = (el.constructor as typeof WcMoney).styles;
    const cssText = Array.isArray(styles)
      ? styles.map((s) => String(s)).join('\n')
      : String(styles);
    expect(cssText).toContain('tabular-nums');
  });
});

describePreviewA11y(preview);
