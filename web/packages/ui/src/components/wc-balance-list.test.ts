import { describe, it, expect, afterEach } from 'vitest';
import './wc-balance-list.js';
import type { BalanceRow, WcBalanceList } from './wc-balance-list.js';
import type { WcMoney } from './wc-money.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-balance-list.preview.js';

const items: BalanceRow[] = [
  { name: 'BofA Checking', accountType: 'checking', balance: 4928.01 },
  { name: 'BofA Credit Card', accountType: 'credit_card', balance: -318.49 },
];

async function mount(props: Partial<WcBalanceList> = {}): Promise<WcBalanceList> {
  const el = document.createElement('wc-balance-list');
  Object.assign(el, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function rows(el: WcBalanceList): Element[] {
  return [...(el.shadowRoot?.querySelectorAll('tbody tr') ?? [])];
}

describe('wc-balance-list', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders one row per account', async () => {
    expect(rows(await mount({ items })).length).toBe(2);
  });

  it('names each account and its type', async () => {
    const el = await mount({ items });
    const text = rows(el)[0]?.textContent ?? '';
    expect(text).toContain('BofA Checking');
    expect(text).toContain('checking');
  });

  it('hands each balance to wc-money', async () => {
    const el = await mount({ items });
    const amounts = [...(el.shadowRoot?.querySelectorAll<WcMoney>('tbody wc-money') ?? [])];
    expect(amounts.map((m) => m.amount)).toEqual([4928.01, -318.49]);
  });

  it('omits the totals row when no total is given', async () => {
    const el = await mount({ items });
    expect(el.shadowRoot?.querySelector('tfoot')).toBeNull();
  });

  it('renders the totals row when a total is given', async () => {
    const el = await mount({ items, total: 4609.52 });
    const foot = el.shadowRoot?.querySelector<WcMoney>('tfoot wc-money');
    expect(foot?.amount).toBe(4609.52);
  });

  it('renders a total of zero rather than treating it as absent', async () => {
    const el = await mount({ items, total: 0 });
    expect(el.shadowRoot?.querySelector('tfoot')).not.toBeNull();
  });

  it('shows an empty message instead of an empty table', async () => {
    const el = await mount({ items: [] });
    expect(el.shadowRoot?.querySelector('table')).toBeNull();
    expect(el.shadowRoot?.querySelector('.empty')?.textContent?.trim()).toBe(
      'No accounts yet.',
    );
  });

  it('shows a spinner while loading', async () => {
    const el = await mount({ items, loading: true });
    expect(el.shadowRoot?.querySelector('wc-spinner')).not.toBeNull();
    expect(el.shadowRoot?.querySelector('table')).toBeNull();
  });

  it('shows the error instead of stale rows and retries', async () => {
    const el = await mount({ items, error: 'boom' });
    expect(el.shadowRoot?.querySelector('table')).toBeNull();
    let fired = 0;
    el.addEventListener('nc-retry', () => (fired += 1));
    el.shadowRoot?.querySelector<HTMLButtonElement>('.retry')?.click();
    expect(fired).toBe(1);
  });

  it('gives the amount column a header association', async () => {
    const el = await mount({ items });
    const headers = [...(el.shadowRoot?.querySelectorAll('thead th') ?? [])].map(
      (th) => th.getAttribute('scope'),
    );
    expect(headers).toEqual(['col', 'col']);
    expect(rows(el)[0]?.querySelector('th')?.getAttribute('scope')).toBe('row');
  });
});

describePreviewA11y(preview);
