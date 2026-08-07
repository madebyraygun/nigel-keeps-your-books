import { describe, it, expect, afterEach } from 'vitest';
import './wc-reconcile-result.js';
import type { WcReconcileResult } from './wc-reconcile-result.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-reconcile-result.preview.js';

async function mount(props: Partial<WcReconcileResult> = {}): Promise<WcReconcileResult> {
  const el = document.createElement('wc-reconcile-result');
  Object.assign(
    el,
    {
      account: 'BofA Checking',
      month: '2025-02',
      isReconciled: true,
      statementBalance: 4928.01,
      calculatedBalance: 4928.01,
      discrepancy: 0,
    },
    props,
  );
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function labels(el: WcReconcileResult): string[] {
  return [...(el.shadowRoot?.querySelectorAll('dt') ?? [])].map(
    (node) => node.textContent?.trim() ?? '',
  );
}

afterEach(() => {
  document.body.innerHTML = '';
});

describe('wc-reconcile-result', () => {
  it('says Reconciled! and shows both balances', async () => {
    const el = await mount();

    const notice = el.shadowRoot?.querySelector('wc-notice-bar');
    expect(notice?.getAttribute('variant')).toBe('success');
    expect(notice?.getAttribute('message')).toBe('Reconciled!');

    // The TUI prints only the calculated figure; both is what makes a zero
    // discrepancy legible rather than merely asserted.
    expect(labels(el)).toEqual(['Account', 'Month', 'Statement', 'Calculated']);
  });

  it('names the difference and emphasises it when the month does not balance', async () => {
    const el = await mount({
      isReconciled: false,
      month: '2025-03',
      statementBalance: 5000,
      calculatedBalance: 4871.44,
      discrepancy: 128.56,
    });

    const notice = el.shadowRoot?.querySelector('wc-notice-bar');
    expect(notice?.getAttribute('variant')).toBe('danger');
    expect(notice?.getAttribute('message')).toBe('Discrepancy');

    expect(labels(el)).toContain('Difference');
    // Emphasis carried by markup, not only by colour.
    const difference = el.shadowRoot?.querySelector('dt.difference');
    expect(difference).not.toBeNull();
  });

  it('renders every amount through wc-money rather than formatting its own', async () => {
    const el = await mount({ isReconciled: false, discrepancy: 128.56 });
    const amounts = el.shadowRoot?.querySelectorAll('wc-money');
    expect(amounts?.length).toBe(3);
  });

  it('carries the account and month it was asked about', async () => {
    const el = await mount({ account: 'Line of Credit', month: '2025-06' });
    const values = [...(el.shadowRoot?.querySelectorAll('dd') ?? [])].map(
      (node) => node.textContent?.trim() ?? '',
    );
    expect(values[0]).toBe('Line of Credit');
    expect(values[1]).toBe('2025-06');
  });
});

describePreviewA11y(preview);
