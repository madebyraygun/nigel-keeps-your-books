import { describe, it, expect, afterEach, vi } from 'vitest';
import './wc-nav-sidebar.js';
import type { WcNavSidebar } from './wc-nav-sidebar.js';
import { NAV_ITEMS, NAV_ITEMS_WITH_DISABLED } from './__mocks__/nav.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-nav-sidebar.preview.js';

async function mount(props: Partial<WcNavSidebar> = {}): Promise<WcNavSidebar> {
  const el = document.createElement('wc-nav-sidebar');
  Object.assign(el, { items: NAV_ITEMS, active: 'dashboard' }, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function button(el: WcNavSidebar, id: string): HTMLButtonElement | null {
  return el.shadowRoot?.querySelector(`[data-nav="${id}"]`) ?? null;
}

describe('wc-nav-sidebar', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders one button per item', async () => {
    const el = await mount();
    expect(el.shadowRoot?.querySelectorAll('[data-nav]').length).toBe(NAV_ITEMS.length);
  });

  it('labels the navigation landmark', async () => {
    const el = await mount();
    expect(el.shadowRoot?.querySelector('nav')?.getAttribute('aria-label')).toBe('Primary');
  });

  it('marks the active item as the current page', async () => {
    const el = await mount({ active: 'register' });
    expect(button(el, 'register')?.getAttribute('aria-current')).toBe('page');
    expect(button(el, 'dashboard')?.getAttribute('aria-current')).toBe('false');
  });

  it('emits nc-navigate with the item id', async () => {
    const el = await mount();
    const spy = vi.fn();
    el.addEventListener('nc-navigate', spy);

    button(el, 'register')?.click();

    expect(spy).toHaveBeenCalledOnce();
    expect(spy.mock.calls[0][0].detail).toEqual({ id: 'register' });
  });

  it('bubbles and composes the event so the app can listen at the shell', async () => {
    const el = await mount();
    const spy = vi.fn();
    document.body.addEventListener('nc-navigate', spy);
    button(el, 'review')?.click();
    expect(spy).toHaveBeenCalledOnce();
    document.body.removeEventListener('nc-navigate', spy);
  });

  it('does not navigate from a disabled item', async () => {
    const el = await mount({ items: NAV_ITEMS_WITH_DISABLED });
    const spy = vi.fn();
    el.addEventListener('nc-navigate', spy);

    button(el, 'register')?.click();

    expect(spy).not.toHaveBeenCalled();
  });

  it('exposes disabled state to assistive tech', async () => {
    const el = await mount({ items: NAV_ITEMS_WITH_DISABLED });
    expect(button(el, 'register')?.getAttribute('aria-disabled')).toBe('true');
    expect(button(el, 'dashboard')?.getAttribute('aria-disabled')).toBe('false');
  });

  it('keeps the label reachable when collapsed', async () => {
    const el = await mount({ collapsed: true });
    // The text is hidden by CSS, so the title attribute is what is left.
    expect(button(el, 'register')?.getAttribute('title')).toBe('Register');
  });

  it('reflects collapsed so CSS can key off it', async () => {
    const el = await mount({ collapsed: true });
    expect(el.hasAttribute('collapsed')).toBe(true);
  });

  it('renders the icon element named by an item', async () => {
    const el = await mount();
    expect(button(el, 'register')?.querySelector('wc-icon-register')).toBeTruthy();
  });

  it('renders items without icons', async () => {
    const el = await mount({ items: [{ id: 'x', label: 'X' }] });
    expect(button(el, 'x')?.textContent?.trim()).toBe('X');
  });
});

describePreviewA11y(preview);
