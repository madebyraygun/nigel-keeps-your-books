import { describe, it, expect, afterEach } from 'vitest';
import './wc-line-items.js';
import {
  isBlankLineItem,
  lineItemAmount,
  lineItemsSubtotal,
  parseLineNumber,
  type LineItemValue,
  type NcLineItemsChangeDetail,
  type WcLineItems,
} from './wc-line-items.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-line-items.preview.js';

const ITEMS: LineItemValue[] = [
  { description: 'Consulting', quantity: '10', unitAmount: '150' },
  { description: 'Hosting', quantity: '1', unitAmount: '350' },
];

async function mount(props: Partial<WcLineItems> = {}): Promise<WcLineItems> {
  const el = document.createElement('wc-line-items');
  Object.assign(el, { items: ITEMS }, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function changes(el: WcLineItems): LineItemValue[][] {
  const seen: LineItemValue[][] = [];
  el.addEventListener('nc-line-items-change', (event) =>
    seen.push((event as CustomEvent<NcLineItemsChangeDetail>).detail.items),
  );
  return seen;
}

describe('line item arithmetic', () => {
  it('reads a typed figure, commas and all', () => {
    expect(parseLineNumber('1,250.50')).toBe(1250.5);
    expect(parseLineNumber('  12 ')).toBe(12);
  });

  it('answers null for a field that is empty or not a number', () => {
    expect(parseLineNumber('')).toBeNull();
    expect(parseLineNumber('lots')).toBeNull();
    expect(parseLineNumber('Infinity')).toBeNull();
  });

  it('computes a row only when both figures are readable', () => {
    expect(lineItemAmount({ description: 'x', quantity: '10', unitAmount: '150' })).toBe(1500);
    expect(lineItemAmount({ description: 'x', quantity: '', unitAmount: '150' })).toBeNull();
  });

  it('refuses an amount that overflows to infinity', () => {
    // The `validate_items` reasoning: 1e308 * 1e308 is infinity, and serde
    // renders a non-finite float as null against a number.
    expect(
      lineItemAmount({ description: 'x', quantity: '1e308', unitAmount: '1e308' }),
    ).toBeNull();
  });

  it('sums the readable rows and ignores the rest', () => {
    expect(lineItemsSubtotal(ITEMS)).toBe(1850);
    expect(
      lineItemsSubtotal([...ITEMS, { description: '', quantity: '', unitAmount: '' }]),
    ).toBe(1850);
  });

  it('calls a row blank when nothing has been typed into it', () => {
    expect(isBlankLineItem({ description: '', quantity: '1', unitAmount: '' })).toBe(true);
    expect(isBlankLineItem({ description: 'x', quantity: '1', unitAmount: '' })).toBe(false);
  });
});

describe('wc-line-items', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders one editable row per item with a computed amount', async () => {
    const el = await mount();
    expect(el.shadowRoot?.querySelectorAll('tbody tr')).toHaveLength(2);
    const amounts = [...(el.shadowRoot?.querySelectorAll('tbody wc-money') ?? [])].map(
      (money) => (money as HTMLElement & { amount: number }).amount,
    );
    expect(amounts).toEqual([1500, 350]);
  });

  it('shows a live subtotal, and a total only when given one', async () => {
    const bare = await mount();
    expect(bare.shadowRoot?.querySelector('tr[data-emphasis="total"]')).toBeNull();
    const totalled = await mount({ total: 1850 });
    expect(totalled.shadowRoot?.querySelector('tr[data-emphasis="total"]')).toBeTruthy();

    const subtotal = bare.shadowRoot?.querySelector(
      'tr[data-emphasis="subtotal"] wc-money',
    ) as HTMLElement & { amount: number };
    expect(subtotal.amount).toBe(1850);
  });

  it('emits the whole array on every edit', async () => {
    const el = await mount();
    const seen = changes(el);

    const input = el.shadowRoot?.querySelectorAll<HTMLInputElement>('[data-description]')[1];
    input!.value = 'Managed hosting';
    input!.dispatchEvent(new Event('input'));

    expect(seen).toHaveLength(1);
    expect(seen[0][1].description).toBe('Managed hosting');
    expect(seen[0][0]).toEqual(ITEMS[0]);
  });

  it('adds and removes rows', async () => {
    const el = await mount();
    const seen = changes(el);

    el.shadowRoot?.querySelector<HTMLElement>('[data-add-row]')?.click();
    expect(seen[0]).toHaveLength(3);

    el.shadowRoot?.querySelectorAll<HTMLElement>('[data-remove]')[0]?.click();
    expect(seen[1]).toEqual([ITEMS[1]]);
  });

  it('reorders with up and down rather than a drag handle', async () => {
    const el = await mount();
    const seen = changes(el);

    el.shadowRoot?.querySelectorAll<HTMLElement>('[data-down]')[0]?.click();
    expect(seen[0].map((item) => item.description)).toEqual(['Hosting', 'Consulting']);

    el.shadowRoot?.querySelectorAll<HTMLElement>('[data-up]')[1]?.click();
    expect(seen[1].map((item) => item.description)).toEqual(['Hosting', 'Consulting']);
  });

  it('disables the ends of the list rather than wrapping around', async () => {
    const el = await mount();
    const ups = el.shadowRoot?.querySelectorAll('[data-up]');
    const downs = el.shadowRoot?.querySelectorAll('[data-down]');
    expect(ups?.[0].hasAttribute('disabled')).toBe(true);
    expect(downs?.[1].hasAttribute('disabled')).toBe(true);
  });

  it('names each row button after the row it acts on', async () => {
    // A column of buttons that all read "Remove" is unusable with a screen
    // reader — the same reason ManagerRow carries a label.
    const el = await mount();
    const labels = [...(el.shadowRoot?.querySelectorAll('[data-remove]') ?? [])].map(
      (button) => button.querySelector('.sr-only')?.textContent,
    );
    expect(labels).toEqual(['Remove Consulting', 'Remove Hosting']);
  });

  it('puts each error beside the field it belongs to', async () => {
    const el = await mount({
      errors: [{ description: 'Description is required', quantity: 'Quantity must be a number' }],
    });
    const messages = [...(el.shadowRoot?.querySelectorAll('.error') ?? [])].map((p) =>
      p.textContent?.trim(),
    );
    expect(messages).toEqual(['Description is required', 'Quantity must be a number']);
  });

  it('offers no editing at all in readonly mode', async () => {
    const el = await mount({ readonly: true, total: 1850 });
    expect(el.shadowRoot?.querySelector('input')).toBeNull();
    expect(el.shadowRoot?.querySelector('[data-add-row]')).toBeNull();
    expect(el.shadowRoot?.querySelector('[data-remove]')).toBeNull();
  });

  it('disables every control while a save is in flight', async () => {
    const el = await mount({ disabled: true });
    const inputs = [...(el.shadowRoot?.querySelectorAll('input') ?? [])];
    expect(inputs.every((input) => input.disabled)).toBe(true);
    expect(el.shadowRoot?.querySelector('[data-add-row]')?.hasAttribute('disabled')).toBe(
      true,
    );
  });
});

describePreviewA11y(preview);
