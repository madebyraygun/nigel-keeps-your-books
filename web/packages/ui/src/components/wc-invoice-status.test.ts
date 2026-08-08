import { describe, it, expect, afterEach } from 'vitest';
import { colorCss, colorDarkCss } from '@nigel/theme';
import './wc-invoice-status.js';
import {
  INVOICE_STATUS_WORDS,
  WcInvoiceStatus,
} from './wc-invoice-status.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-invoice-status.preview.js';

async function mount(status: string): Promise<WcInvoiceStatus> {
  const el = document.createElement('wc-invoice-status');
  el.status = status;
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

/** Every colour custom property the chip reads, in source order. */
function colorTokensUsed(): string[] {
  const css = [WcInvoiceStatus.styles].flat().map(String).join('\n');
  return [...css.matchAll(/(?:^|\s)color:\s*var\(([^)]*)\)/gm)].map((match) =>
    match[1].trim(),
  );
}

describe('the status chip’s colours', () => {
  const theme = `${colorCss}\n${colorDarkCss}`;

  it('names only tokens @nigel/theme defines, in both schemes', () => {
    // The bug this catches: `--nc-color-warning` does not exist, so its
    // literal fallback always won — a colour the theme's contrast test could
    // not see and never held to AA.
    const used = colorTokensUsed();
    expect(used.length).toBeGreaterThan(0);

    for (const token of used) {
      expect(theme, `${token} is not defined by @nigel/theme`).toContain(`${token}:`);
      expect(colorDarkCss.toString(), `${token} has no dark value`).toContain(
        `${token}:`,
      );
    }
  });

  it('carries no literal fallback that could win over a token', () => {
    // A fallback only ever renders when the token is missing, which is exactly
    // the case the contrast test cannot reach.
    expect(colorTokensUsed().filter((token) => token.includes(','))).toEqual([]);
  });

  it('reads partial as flagged rather than inventing a warning colour', () => {
    expect(colorTokensUsed()).toContain('--nc-color-flagged');
  });
});

describe('wc-invoice-status', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('knows the six statuses the data layer derives', () => {
    expect(INVOICE_STATUS_WORDS).toEqual([
      'draft',
      'sent',
      'partial',
      'paid',
      'overdue',
      'void',
    ]);
  });

  it.each(INVOICE_STATUS_WORDS)('renders %s as a glyph and the word', async (status) => {
    const el = await mount(status);
    const chip = el.shadowRoot?.querySelector('.chip');
    expect(chip?.getAttribute('data-status')).toBe(status);
    expect(chip?.querySelector('.word')?.textContent).toBe(status);
    // The glyph is decoration: the word is what a screen reader announces.
    expect(chip?.querySelector('.glyph')?.getAttribute('aria-hidden')).toBe('true');
    expect(chip?.querySelector('.glyph')?.textContent?.trim()).not.toBe('');
  });

  it('renders a status it has never seen rather than blanking it', async () => {
    // `invoices.status` has no CHECK constraint, so a row written by the
    // InvoiceShelf importer or by hand cannot be assumed to be one of the six.
    const el = await mount('imported');
    expect(el.shadowRoot?.querySelector('.word')?.textContent).toBe('imported');
    expect(el.shadowRoot?.querySelector('.glyph')?.textContent?.trim()).toBe('•');
  });
});

describePreviewA11y(preview);
