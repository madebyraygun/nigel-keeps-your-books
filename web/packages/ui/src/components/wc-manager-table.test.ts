import { describe, it, expect, afterEach } from 'vitest';
import './wc-manager-table.js';
import type { WcManagerTable, NcManagerActionDetail } from './wc-manager-table.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-manager-table.preview.js';

const columns = [
  { key: 'name', label: 'Name' },
  { key: 'type', label: 'Type' },
  { key: 'hits', label: 'Hits', align: 'end' as const },
];

const rows = [
  { id: 1, label: 'BofA Checking', cells: ['BofA Checking', 'Checking', 12] },
  { id: 2, label: 'Gusto Payroll', cells: ['Gusto Payroll', 'Payroll', null] },
];

const actions = [
  { name: 'edit', label: 'Rename', icon: 'wc-icon-edit' },
  { name: 'delete', label: 'Delete', icon: 'wc-icon-trash', variant: 'danger' as const },
];

async function mount(props: Partial<WcManagerTable> = {}): Promise<WcManagerTable> {
  const el = document.createElement('wc-manager-table');
  Object.assign(el, { caption: 'Accounts', columns, rows, actions }, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function cells(el: WcManagerTable, rowId: number): string[] {
  const row = el.shadowRoot?.querySelector(`tr[data-row="${rowId}"]`);
  return [...(row?.querySelectorAll('td') ?? [])].map((td) =>
    (td.textContent ?? '').trim(),
  );
}

describe('wc-manager-table', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders a header per column plus one for the actions', async () => {
    const el = await mount();
    const headers = [...(el.shadowRoot?.querySelectorAll('th') ?? [])].map((th) =>
      th.textContent?.trim(),
    );
    expect(headers).toEqual(['Name', 'Type', 'Hits', 'Actions']);
  });

  it('renders rows in the order given', async () => {
    const el = await mount();
    const names = [...(el.shadowRoot?.querySelectorAll('tbody tr') ?? [])].map(
      (tr) => tr.querySelector('td')?.textContent?.trim(),
    );
    expect(names).toEqual(['BofA Checking', 'Gusto Payroll']);
  });

  it('renders a null cell as an em dash rather than as nothing', async () => {
    // An empty cell reads as "we did not render this"; the dash says the
    // account has no value there, which is what the register does too.
    const el = await mount();
    expect(cells(el, 2)[2]).toBe('—');
  });

  it('names the row in every action label', async () => {
    const el = await mount();
    const labels = [
      ...(el.shadowRoot?.querySelectorAll('tr[data-row="1"] [data-action]') ?? []),
    ].map((button) => button.getAttribute('aria-label'));
    expect(labels).toEqual(['Rename BofA Checking', 'Delete BofA Checking']);
  });

  it('emits nc-manager-action with the action name and row id', async () => {
    const el = await mount();
    const seen: NcManagerActionDetail[] = [];
    el.addEventListener('nc-manager-action', (event) =>
      seen.push((event as CustomEvent<NcManagerActionDetail>).detail),
    );

    el.shadowRoot
      ?.querySelector<HTMLElement>('tr[data-row="2"] [data-action="delete"]')
      ?.click();

    expect(seen).toEqual([{ action: 'delete', id: 2 }]);
  });

  it('disables only the busy row', async () => {
    const el = await mount({ busyId: 1 });
    const busy = el.shadowRoot?.querySelector('tr[data-row="1"] [data-action="edit"]');
    const idle = el.shadowRoot?.querySelector('tr[data-row="2"] [data-action="edit"]');
    expect(busy?.hasAttribute('disabled')).toBe(true);
    expect(idle?.hasAttribute('disabled')).toBe(false);
    expect(
      el.shadowRoot?.querySelector('tr[data-row="1"]')?.getAttribute('aria-busy'),
    ).toBe('true');
  });

  it('drops the actions column entirely when there are no actions', async () => {
    const el = await mount({ actions: [] });
    const headers = [...(el.shadowRoot?.querySelectorAll('th') ?? [])].map((th) =>
      th.textContent?.trim(),
    );
    expect(headers).toEqual(['Name', 'Type', 'Hits']);
    expect(el.shadowRoot?.querySelector('[data-action]')).toBeNull();
  });

  it('captions the table for screen readers', async () => {
    const el = await mount();
    expect(el.shadowRoot?.querySelector('caption')?.textContent).toBe('Accounts');
  });
});

describePreviewA11y(preview);
