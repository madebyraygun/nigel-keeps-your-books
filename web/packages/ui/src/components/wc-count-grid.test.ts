import { describe, it, expect, afterEach } from 'vitest';
import './wc-count-grid.js';
import type { WcCountGrid } from './wc-count-grid.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-count-grid.preview.js';

async function mount(props: Partial<WcCountGrid> = {}): Promise<WcCountGrid> {
  const el = document.createElement('wc-count-grid');
  Object.assign(el, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe('wc-count-grid', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('pairs every label with its value', async () => {
    const el = await mount({
      items: [
        { label: 'Imported', value: 42 },
        { label: 'Malformed', value: 1 },
      ],
    });

    const labels = [...(el.shadowRoot?.querySelectorAll('dt') ?? [])].map((dt) =>
      dt.textContent?.trim(),
    );
    const values = [...(el.shadowRoot?.querySelectorAll('dd') ?? [])].map((dd) =>
      dd.textContent?.trim(),
    );

    expect(labels).toEqual(['Imported', 'Malformed']);
    expect(values).toEqual(['42', '1']);
  });

  it('renders zero rather than leaving the value blank', async () => {
    // A falsy count is the whole point of a result panel: "0 malformed" is
    // information, and an empty cell would read as "not measured".
    const el = await mount({ items: [{ label: 'Malformed', value: 0 }] });
    expect(el.shadowRoot?.querySelector('dd')?.textContent?.trim()).toBe('0');
  });

  it('applies the emphasis class', async () => {
    const el = await mount({
      items: [
        { label: 'Imported', value: 42, emphasis: 'good' },
        { label: 'Still flagged', value: 6, emphasis: 'warn' },
        { label: 'Skipped', value: 3 },
      ],
    });

    const classes = [...(el.shadowRoot?.querySelectorAll('dd') ?? [])].map(
      (dd) => dd.className,
    );
    expect(classes).toEqual(['good', 'warn', 'default']);
  });

  it('renders a hint when one is given', async () => {
    const el = await mount({
      items: [{ label: 'Still flagged', value: 6, hint: 'across the ledger' }],
    });
    expect(el.shadowRoot?.querySelector('.hint')?.textContent).toBe(
      'across the ledger',
    );
  });

  it('renders nothing but an empty list with no items', async () => {
    const el = await mount();
    expect(el.shadowRoot?.querySelectorAll('.item')).toHaveLength(0);
  });
});

describePreviewA11y(preview);
