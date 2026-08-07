import { describe, it, expect, afterEach } from 'vitest';
import './wc-link-grid.js';
import type { LinkGridItem, WcLinkGrid } from './wc-link-grid.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-link-grid.preview.js';

const items: LinkGridItem[] = [
  {
    href: '#/reports?report=pnl',
    label: 'Profit and loss',
    description: 'Income and expenses by category.',
    icon: 'wc-icon-report',
  },
  { href: '#/reports?report=tax', label: 'Tax summary' },
];

async function mount(props: Partial<WcLinkGrid> = {}): Promise<WcLinkGrid> {
  const el = document.createElement('wc-link-grid');
  Object.assign(el, { label: 'Reports', items, ...props });
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function all<T extends Element>(el: WcLinkGrid, selector: string): T[] {
  return [...(el.shadowRoot?.querySelectorAll<T>(selector) ?? [])];
}

function query<T extends Element>(el: WcLinkGrid, selector: string): T | null {
  return el.shadowRoot?.querySelector<T>(selector) ?? null;
}

describe('wc-link-grid', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders an anchor per item, in order', async () => {
    const el = await mount();
    expect(all<HTMLAnchorElement>(el, 'a').map((a) => a.getAttribute('href'))).toEqual([
      '#/reports?report=pnl',
      '#/reports?report=tax',
    ]);
  });

  it('renders the label of each item', async () => {
    const el = await mount();
    expect(all(el, '.label').map((span) => span.textContent?.trim())).toEqual([
      'Profit and loss',
      'Tax summary',
    ]);
  });

  it('renders a description only when the item has one', async () => {
    const el = await mount();
    expect(all(el, '.description')).toHaveLength(1);
    expect(query(el, '.description')?.textContent?.trim()).toBe(
      'Income and expenses by category.',
    );
  });

  it('names the navigation landmark', async () => {
    const el = await mount({ label: 'Reports' });
    expect(query(el, 'nav')?.getAttribute('aria-label')).toBe('Reports');
  });

  it('hides the decorative icon from assistive technology', async () => {
    const el = await mount();
    // The icon duplicates the label beside it; announcing it twice is noise.
    expect(query(el, 'wc-icon-report')?.getAttribute('aria-hidden')).toBe('true');
  });

  it('renders an empty list without failing', async () => {
    const el = await mount({ items: [] });
    expect(all(el, 'a')).toHaveLength(0);
    expect(query(el, 'ul')).not.toBeNull();
  });
});

describePreviewA11y(preview);
