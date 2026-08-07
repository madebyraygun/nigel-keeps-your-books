import { describe, it, expect, afterEach } from 'vitest';
import './wc-sample-table.js';
import type { WcSampleTable } from './wc-sample-table.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-sample-table.preview.js';

async function mount(props: Partial<WcSampleTable> = {}): Promise<WcSampleTable> {
  const el = document.createElement('wc-sample-table');
  Object.assign(el, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

const ROWS = [
  { date: '2025-04-01', description: 'ACME CORP', amount: 3000 },
  { date: '2025-04-03', description: 'ADOBE', amount: -59.99 },
];

describe('wc-sample-table', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders one row per sample', async () => {
    const el = await mount({ rows: ROWS });
    expect(el.shadowRoot?.querySelectorAll('tbody tr')).toHaveLength(2);
  });

  it('renders the date and description as written', async () => {
    const el = await mount({ rows: ROWS });
    const first = el.shadowRoot?.querySelector('tbody tr');
    expect(first?.querySelector('.date')?.textContent?.trim()).toBe('2025-04-01');
    expect(first?.querySelector('.description')?.textContent?.trim()).toBe('ACME CORP');
  });

  it('renders amounts through wc-money', async () => {
    const el = await mount({ rows: ROWS });
    const monies = [...(el.shadowRoot?.querySelectorAll('wc-money') ?? [])] as (
      HTMLElement & { amount: number }
    )[];
    expect(monies.map((m) => m.amount)).toEqual([3000, -59.99]);
  });

  it('shows the empty message instead of a table when there are no rows', async () => {
    const el = await mount({ emptyMessage: 'Nothing parsed.' });
    expect(el.shadowRoot?.querySelector('table')).toBeNull();
    expect(el.shadowRoot?.querySelector('.empty')?.textContent).toBe('Nothing parsed.');
  });

  it('renders the caption when given one', async () => {
    const el = await mount({ rows: ROWS, caption: 'First 5 rows' });
    expect(el.shadowRoot?.querySelector('caption')?.textContent?.trim()).toBe(
      'First 5 rows',
    );
  });

  it('omits the caption element when there is no caption', async () => {
    const el = await mount({ rows: ROWS });
    expect(el.shadowRoot?.querySelector('caption')).toBeNull();
  });
});

describePreviewA11y(preview);
