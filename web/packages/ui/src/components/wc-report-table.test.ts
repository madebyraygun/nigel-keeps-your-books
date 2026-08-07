import { describe, it, expect, afterEach } from 'vitest';
import './wc-report-table.js';
import type { ReportColumn, ReportTableRow, WcReportTable } from './wc-report-table.js';
import type { WcMoney } from './wc-money.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-report-table.preview.js';

const columns: ReportColumn[] = [
  { key: 'name', label: 'Category', kind: 'text' },
  { key: 'amount', label: 'Amount', kind: 'money' },
];

async function mount(props: Partial<WcReportTable> = {}): Promise<WcReportTable> {
  const el = document.createElement('wc-report-table');
  Object.assign(el, { caption: 'Profit and loss', columns, ...props });
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function all<T extends Element>(el: WcReportTable, selector: string): T[] {
  return [...(el.shadowRoot?.querySelectorAll<T>(selector) ?? [])];
}

function query<T extends Element>(el: WcReportTable, selector: string): T | null {
  return el.shadowRoot?.querySelector<T>(selector) ?? null;
}

describe('wc-report-table', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders a header cell per column', async () => {
    const el = await mount({ rows: [{ cells: { name: 'Client Services', amount: 10 } }] });
    expect(all(el, 'thead th').map((th) => th.textContent?.trim())).toEqual([
      'Category',
      'Amount',
    ]);
  });

  it('renders a body row per row, in order', async () => {
    const rows: ReportTableRow[] = [
      { cells: { name: 'First', amount: 1 } },
      { cells: { name: 'Second', amount: 2 } },
    ];
    const el = await mount({ rows });
    expect(all(el, 'tbody td.label').map((td) => td.textContent?.trim())).toEqual([
      'First',
      'Second',
    ]);
  });

  it('hands money cells to wc-money rather than formatting them itself', async () => {
    const el = await mount({ rows: [{ cells: { name: 'Fees', amount: -12.5 } }] });
    const money = query<WcMoney>(el, 'wc-money');
    expect(money?.amount).toBe(-12.5);
    expect(money?.variant).toBe('signed');
  });

  it('prints moneyAbs cells as a magnitude in plain ink', async () => {
    const el = await mount({
      columns: [
        { key: 'name', label: 'Category', kind: 'text' },
        { key: 'amount', label: 'Amount', kind: 'moneyAbs' },
      ],
      rows: [{ cells: { name: 'Fees', amount: -12.5 } }],
    });
    const money = query<WcMoney>(el, 'wc-money');
    // The magnitude is what `text.rs` prints for an expense column; the column
    // heading is what says which way the money went.
    expect(money?.amount).toBe(12.5);
    expect(money?.variant).toBe('plain');
  });

  it('formats percent and count without a money element', async () => {
    const el = await mount({
      columns: [
        { key: 'name', label: 'Category', kind: 'text' },
        { key: 'pct', label: '%', kind: 'percent' },
        { key: 'count', label: 'Count', kind: 'count' },
      ],
      rows: [{ cells: { name: 'Software', pct: 62.35, count: 3 } }],
    });
    const cells = all(el, 'tbody td').map((td) => td.textContent?.trim());
    expect(cells).toEqual(['Software', '62.4%', '3']);
    expect(query(el, 'wc-money')).toBeNull();
  });

  it('rounds a percent tie the way `{:.1}` does', async () => {
    const el = await mount({
      columns: [
        { key: 'name', label: 'Category', kind: 'text' },
        { key: 'pct', label: '%', kind: 'percent' },
      ],
      rows: [{ cells: { name: 'Software', pct: 12.25 } }],
    });
    expect(all(el, 'tbody td').map((td) => td.textContent?.trim())).toEqual([
      'Software',
      '12.2%',
    ]);
  });

  it('lets a row override the column format', async () => {
    // The P&L's expense band, printed as a magnitude in a signed column.
    const el = await mount({
      rows: [
        { cells: { name: 'Client Services', amount: 8700 } },
        { cells: { name: 'Fees', amount: -24 }, cellKinds: { amount: 'moneyAbs' } },
      ],
    });
    const money = all<WcMoney>(el, 'wc-money');
    expect(money.map((m) => m.amount)).toEqual([8700, 24]);
    expect(money.map((m) => m.variant)).toEqual(['signed', 'plain']);
  });

  it('spans a section row across the table', async () => {
    const el = await mount({
      rows: [{ cells: { name: 'Income' }, emphasis: 'section' }],
    });
    const cell = query<HTMLTableCellElement>(el, 'tr[data-emphasis="section"] td');
    expect(cell?.getAttribute('colspan')).toBe('2');
    expect(cell?.textContent?.trim()).toBe('Income');
  });

  it('marks subtotal and total rows so the styling is not guesswork', async () => {
    const el = await mount({
      rows: [
        { cells: { name: 'Total Income', amount: 10 }, emphasis: 'subtotal' },
        { cells: { name: 'Net', amount: 8 }, emphasis: 'total' },
      ],
    });
    expect(all(el, 'tbody tr').map((tr) => tr.getAttribute('data-emphasis'))).toEqual([
      'subtotal',
      'total',
    ]);
  });

  it('indents a nested row', async () => {
    const el = await mount({
      rows: [{ cells: { name: 'Client Services', amount: 1 }, indent: 1 }],
    });
    expect(query(el, 'td.label')?.classList.contains('indent-1')).toBe(true);
  });

  it('renders a note after the label', async () => {
    const el = await mount({
      rows: [{ cells: { name: 'Meals', amount: 400 }, note: '(50%)' }],
    });
    expect(query(el, '.note')?.textContent?.trim()).toBe('(50%)');
    expect(query(el, 'td.label')?.textContent?.replace(/\s+/g, ' ').trim()).toBe(
      'Meals (50%)',
    );
  });

  it('turns a row with an href into a link', async () => {
    const el = await mount({
      rows: [{ cells: { name: 'UNKNOWN VENDOR', amount: -240.5 }, href: '#/review?id=6' }],
    });
    const link = query<HTMLAnchorElement>(el, 'td.label a');
    expect(link?.getAttribute('href')).toBe('#/review?id=6');
  });

  it('leaves rows without an href unlinked', async () => {
    const el = await mount({ rows: [{ cells: { name: 'Fees', amount: -12 } }] });
    expect(query(el, 'td.label a')).toBeNull();
  });

  it('names the table with a caption', async () => {
    const el = await mount({
      caption: 'Tax summary',
      rows: [{ cells: { name: 'Fees', amount: -12 } }],
    });
    const caption = query(el, 'caption');
    expect(caption?.textContent?.trim()).toBe('Tax summary');
    expect(caption?.classList.contains('visually-hidden')).toBe(false);
  });

  it('keeps the caption for screen readers when it is visually hidden', async () => {
    const el = await mount({
      captionHidden: true,
      rows: [{ cells: { name: 'Fees', amount: -12 } }],
    });
    // Hidden, not removed: the accessible name has to survive.
    expect(query(el, 'caption')?.classList.contains('visually-hidden')).toBe(true);
    expect(query(el, 'caption')?.textContent?.trim()).toBe('Profit and loss');
  });

  it('aligns numeric columns to the end by default', async () => {
    const el = await mount({ rows: [{ cells: { name: 'Fees', amount: -12 } }] });
    const headers = all<HTMLTableCellElement>(el, 'thead th');
    expect(headers[0]?.classList.contains('end')).toBe(false);
    expect(headers[1]?.classList.contains('end')).toBe(true);
  });

  it('shows a spinner while loading and no table', async () => {
    const el = await mount({ loading: true });
    expect(query(el, 'wc-spinner')).not.toBeNull();
    expect(query(el, 'table')).toBeNull();
  });

  it('shows the error instead of a stale table', async () => {
    const el = await mount({
      rows: [{ cells: { name: 'Fees', amount: -12 } }],
      error: 'Could not reach the nigel server.',
    });
    expect(query(el, '.error')?.textContent?.trim()).toBe(
      'Could not reach the nigel server.',
    );
    expect(query(el, 'table')).toBeNull();
  });

  it('fires nc-retry across the shadow boundary', async () => {
    const el = await mount({ error: 'boom' });
    let fired = 0;
    document.body.addEventListener('nc-retry', () => (fired += 1));
    query<HTMLButtonElement>(el, '.retry')?.click();
    expect(fired).toBe(1);
  });

  it('shows the empty message when there are no rows', async () => {
    const el = await mount({ rows: [], emptyMessage: 'No flagged transactions.' });
    expect(query(el, '.state')?.textContent?.trim()).toBe('No flagged transactions.');
    expect(query(el, 'table')).toBeNull();
  });

  it('renders nothing for a null cell rather than the word null', async () => {
    const el = await mount({
      columns: [
        { key: 'name', label: 'Category', kind: 'text' },
        { key: 'taxLine', label: 'Tax line', kind: 'text' },
      ],
      rows: [{ cells: { name: 'Fees', taxLine: null } }],
    });
    const cells = all(el, 'tbody td').map((td) => td.textContent?.trim());
    expect(cells).toEqual(['Fees', '']);
  });
});

describePreviewA11y(preview);
