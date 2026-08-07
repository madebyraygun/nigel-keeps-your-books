import { describe, it, expect, afterEach, vi } from 'vitest';
import './wc-register-table.js';
import type {
  CategoryOption,
  NcEditCommitDetail,
  NcFlagToggleDetail,
  NcRowEventDetail,
  RegisterTableRow,
  WcRegisterTable,
} from './wc-register-table.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-register-table.preview.js';

const categories: CategoryOption[] = [
  { id: 3, name: 'Consulting income', categoryType: 'income' },
  { id: 12, name: 'Software / Subscriptions', categoryType: 'expense' },
  { id: 21, name: 'Bank fees', categoryType: 'expense' },
];

function fixture(count = 4): RegisterTableRow[] {
  return Array.from({ length: count }, (_, index) => ({
    id: 100 + index,
    date: `2025-03-${String(index + 1).padStart(2, '0')}`,
    description: `TRANSACTION ${index}`,
    amount: index % 2 === 0 ? -10 * (index + 1) : 100 * (index + 1),
    category: index === 0 ? null : 'Bank fees',
    categoryId: index === 0 ? null : 21,
    vendor: index === 0 ? null : 'Someone',
    accountName: 'BofA Checking',
    isFlagged: index === 2,
  }));
}

async function mount(props: Partial<WcRegisterTable> = {}): Promise<WcRegisterTable> {
  const el = document.createElement('wc-register-table');
  Object.assign(el, { rows: fixture(), categories, ...props });
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function rowEls(el: WcRegisterTable): HTMLElement[] {
  return [...(el.shadowRoot?.querySelectorAll<HTMLElement>('tbody tr') ?? [])];
}

function selectedId(el: WcRegisterTable): number | null {
  const row = rowEls(el).find((tr) => tr.getAttribute('aria-selected') === 'true');
  return row ? Number(row.dataset.id) : null;
}

async function press(el: WcRegisterTable, key: string): Promise<void> {
  el.shadowRoot
    ?.querySelector('.scroller')
    ?.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true }));
  await el.updateComplete;
}

function listen<T>(el: WcRegisterTable, type: string): T[] {
  const seen: T[] = [];
  el.addEventListener(type, (event) => seen.push((event as CustomEvent<T>).detail));
  return seen;
}

describe('wc-register-table', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders one row per transaction, in the order given', async () => {
    const el = await mount();
    expect(rowEls(el).map((tr) => Number(tr.dataset.id))).toEqual([100, 101, 102, 103]);
  });

  it('shows an em dash for an uncategorized row and nothing for a missing vendor', async () => {
    const el = await mount();
    const cells = [...(rowEls(el)[0]?.querySelectorAll('td') ?? [])];
    expect(cells[3]?.textContent?.trim()).toBe('—');
    expect(cells[4]?.textContent?.trim()).toBe('');
  });

  it('marks a flagged row for more than color alone', async () => {
    const el = await mount();
    const flagged = rowEls(el)[2];
    expect(flagged?.dataset.flagged).toBe('true');
    expect(
      flagged?.querySelector('button')?.getAttribute('aria-pressed'),
    ).toBe('true');
  });

  it('drops the account column on request and keeps every row the same width', async () => {
    const el = await mount({ showAccount: false, total: 12 });
    const headers = el.shadowRoot?.querySelectorAll('thead th').length;
    const cells = rowEls(el)[0]?.querySelectorAll('td').length;
    expect(headers).toBe(6);
    expect(cells).toBe(6);
  });

  it('renders no wa-* component while nothing is being edited', async () => {
    const el = await mount();
    const tags = [...(el.shadowRoot?.querySelectorAll('*') ?? [])].map((node) =>
      node.tagName.toLowerCase(),
    );
    expect(tags.filter((tag) => tag.startsWith('wa-'))).toEqual([]);
  });

  it('renders the empty state instead of a table', async () => {
    const el = await mount({ rows: [] });
    expect(el.shadowRoot?.querySelector('table')).toBeNull();
    expect(el.shadowRoot?.querySelector('wc-empty-state')).not.toBeNull();
  });

  // -- selection and keyboard ------------------------------------------------

  it('keeps exactly one row in the tab order', async () => {
    const el = await mount({ selectedId: 101 });
    expect(rowEls(el).filter((tr) => tr.tabIndex === 0).map((tr) => tr.dataset.id)).toEqual(
      ['101'],
    );
  });

  it('moves selection with the arrow keys and stops at the ends', async () => {
    const el = await mount({ selectedId: 100 });

    await press(el, 'ArrowDown');
    expect(selectedId(el)).toBe(101);

    await press(el, 'ArrowUp');
    await press(el, 'ArrowUp');
    expect(selectedId(el)).toBe(100);

    await press(el, 'End');
    expect(selectedId(el)).toBe(103);

    await press(el, 'ArrowDown');
    expect(selectedId(el)).toBe(103);

    await press(el, 'Home');
    expect(selectedId(el)).toBe(100);
  });

  it('pages by a screenful, falling back to the TUI page size without layout', async () => {
    const rows = fixture(50);
    const el = await mount({ rows, selectedId: rows[0]?.id });

    await press(el, 'PageDown');
    expect(selectedId(el)).toBe(rows[20]?.id);

    await press(el, 'PageUp');
    expect(selectedId(el)).toBe(rows[0]?.id);
  });

  it('clamps a page jump to the last row', async () => {
    const el = await mount({ selectedId: 100 });
    await press(el, 'PageDown');
    expect(selectedId(el)).toBe(103);
  });

  it('announces selection changes to the host', async () => {
    const el = await mount({ selectedId: 100 });
    const seen = listen<NcRowEventDetail>(el, 'nc-row-select');
    await press(el, 'ArrowDown');
    expect(seen).toEqual([{ id: 101 }]);
  });

  it('selects a row when it takes focus', async () => {
    const el = await mount({ selectedId: 100 });
    const seen = listen<NcRowEventDetail>(el, 'nc-row-select');
    rowEls(el)[2]?.dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    await el.updateComplete;
    expect(seen).toEqual([{ id: 102 }]);
  });

  it('asks the host to edit on Enter and on double click', async () => {
    const el = await mount({ selectedId: 101 });
    const seen = listen<NcRowEventDetail>(el, 'nc-row-activate');

    await press(el, 'Enter');
    rowEls(el)[3]?.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
    await el.updateComplete;

    expect(seen).toEqual([{ id: 101 }, { id: 103 }]);
  });

  it('asks the toolbar for focus on slash', async () => {
    const el = await mount();
    let asked = 0;
    el.addEventListener('nc-search-focus', () => (asked += 1));
    await press(el, '/');
    expect(asked).toBe(1);
  });

  it('leaves the keyboard alone while a row is being edited', async () => {
    const el = await mount({ selectedId: 100, editingId: 100 });
    await press(el, 'ArrowDown');
    expect(selectedId(el)).toBe(100);
  });

  // -- flagging --------------------------------------------------------------

  it('sends the desired flag state rather than a toggle', async () => {
    const el = await mount({ selectedId: 100 });
    const seen = listen<NcFlagToggleDetail>(el, 'nc-flag-toggle');

    await press(el, 'f');
    rowEls(el)[2]?.querySelector('button')?.click();

    expect(seen).toEqual([
      { id: 100, flag: true },
      { id: 102, flag: false },
    ]);
  });

  it('disables the flag button on a row with a write in flight', async () => {
    const el = await mount({ busyId: 100 });
    const button = rowEls(el)[0]?.querySelector('button');
    expect(button?.hasAttribute('disabled')).toBe(true);
    expect(rowEls(el)[0]?.getAttribute('aria-busy')).toBe('true');
  });

  // -- inline editing --------------------------------------------------------

  it('swaps only the category and vendor cells into editors', async () => {
    const el = await mount({ editingId: 101 });
    const editing = rowEls(el)[1];
    expect(editing?.querySelector('input[role="combobox"]')).not.toBeNull();
    expect(editing?.querySelector('wa-input')).not.toBeNull();
    expect(rowEls(el)[0]?.querySelector('input')).toBeNull();
  });

  it('seeds the editors from the row', async () => {
    const el = await mount({ editingId: 101 });
    const input = el.shadowRoot?.querySelector<HTMLInputElement>('.category-input');
    const vendor = el.shadowRoot?.querySelector('wa-input');
    expect(input?.value).toBe('Bank fees');
    expect(vendor?.getAttribute('value')).toBe('Someone');
  });

  it('filters categories case-insensitively over the labelled name', async () => {
    const el = await mount({ editingId: 101 });
    const input = el.shadowRoot?.querySelector<HTMLInputElement>('.category-input');
    if (!input) throw new Error('no category input');

    input.value = 'consult';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    await el.updateComplete;

    const options = [...(el.shadowRoot?.querySelectorAll('[role="option"]') ?? [])];
    expect(options.map((o) => o.textContent?.trim())).toEqual([
      'Consulting income (inc)',
    ]);
  });

  it('commits the chosen category and typed vendor', async () => {
    const el = await mount({ editingId: 101 });
    const seen = listen<NcEditCommitDetail>(el, 'nc-edit-commit');
    const input = el.shadowRoot?.querySelector<HTMLInputElement>('.category-input');
    if (!input) throw new Error('no category input');

    input.value = 'consult';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    await el.updateComplete;
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    await el.updateComplete;

    const vendor = el.shadowRoot?.querySelector<HTMLElement & { value: string }>(
      '.vendor-input',
    );
    if (!vendor) throw new Error('no vendor input');
    vendor.value = 'Northwind';
    vendor.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    await el.updateComplete;

    expect(seen).toEqual([{ id: 101, categoryId: 3, vendor: 'Northwind' }]);
  });

  it('reports an emptied vendor as a clear, not as an untouched field', async () => {
    const el = await mount({ editingId: 101 });
    const seen = listen<NcEditCommitDetail>(el, 'nc-edit-commit');
    const vendor = el.shadowRoot?.querySelector<HTMLElement & { value: string }>(
      '.vendor-input',
    );
    if (!vendor) throw new Error('no vendor input');

    vendor.value = '   ';
    vendor.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    await el.updateComplete;

    expect(seen).toEqual([{ id: 101, categoryId: 21, vendor: null }]);
  });

  it('cancels from either editor without committing', async () => {
    const el = await mount({ editingId: 101 });
    const commits = listen<NcEditCommitDetail>(el, 'nc-edit-commit');
    const cancels = listen<NcRowEventDetail>(el, 'nc-edit-cancel');

    el.shadowRoot
      ?.querySelector('.category-input')
      ?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await el.updateComplete;

    expect(cancels).toEqual([{ id: 101 }]);
    expect(commits).toEqual([]);
  });

  it('offers Save and Cancel for a pointer', async () => {
    const el = await mount({ editingId: 101 });
    const commits = listen<NcEditCommitDetail>(el, 'nc-edit-commit');
    const cancels = listen<NcRowEventDetail>(el, 'nc-edit-cancel');
    const buttons = [
      ...(rowEls(el)[1]?.querySelectorAll<HTMLButtonElement>('.edit-actions button') ??
        []),
    ];

    buttons[0]?.click();
    buttons[1]?.click();
    await el.updateComplete;

    expect(commits.length).toBe(1);
    expect(cancels.length).toBe(1);
  });

  // -- scrolling -------------------------------------------------------------

  it('scrolls a row into view by index and selects it', async () => {
    const el = await mount();
    const scrollIntoView = vi.fn();
    rowEls(el).forEach((row) => {
      row.scrollIntoView = scrollIntoView;
    });

    expect(el.scrollToIndex(2)).toBe(true);
    await el.updateComplete;
    await Promise.resolve();

    expect(selectedId(el)).toBe(102);
    expect(scrollIntoView).toHaveBeenCalledWith({ block: 'center' });
  });

  it('scrolls by transaction id, and reports an id it does not have', async () => {
    const el = await mount();
    expect(el.scrollToRow(103)).toBe(true);
    expect(el.scrollToRow(9999)).toBe(false);
    expect(el.scrollToIndex(99)).toBe(false);
  });

  it('moves selection off a row that filtering removed', async () => {
    const el = await mount({ selectedId: 103 });
    el.rows = fixture(2);
    await el.updateComplete;
    expect(selectedId(el)).toBe(100);
  });

  describe('readonly', () => {
    it('offers no flag button', async () => {
      const el = await mount({ readonly: true });
      expect(el.shadowRoot?.querySelector('.flag button')).toBeNull();
    });

    it('still shows which rows are flagged, with a label rather than colour', async () => {
      const el = await mount({ readonly: true });
      const marks = el.shadowRoot?.querySelectorAll('.flag wc-icon-flag') ?? [];
      // Exactly the one flagged row the fixture carries.
      expect(marks).toHaveLength(1);
      expect(marks[0]?.getAttribute('aria-label')).toBe('Flagged');
    });

    it('emits no flag toggle from the f key', async () => {
      const el = await mount({ readonly: true, selectedId: 102 });
      const seen = listen<NcFlagToggleDetail>(el, 'nc-flag-toggle');
      await press(el, 'f');
      expect(seen).toEqual([]);
    });

    it('emits no row activation from Enter', async () => {
      const el = await mount({ readonly: true, selectedId: 102 });
      const seen = listen<NcRowEventDetail>(el, 'nc-row-activate');
      await press(el, 'Enter');
      expect(seen).toEqual([]);
    });

    it('emits no row activation from a double click', async () => {
      const el = await mount({ readonly: true });
      const seen = listen<NcRowEventDetail>(el, 'nc-row-activate');
      rowEls(el)[1]?.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
      await el.updateComplete;
      expect(seen).toEqual([]);
    });

    it('keeps arrow-key selection, because reading still wants a cursor', async () => {
      const el = await mount({ readonly: true, selectedId: 100 });
      await press(el, 'ArrowDown');
      expect(selectedId(el)).toBe(101);
    });
  });
});

describePreviewA11y(preview);
