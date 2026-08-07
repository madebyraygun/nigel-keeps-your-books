import { describe, it, expect, afterEach } from 'vitest';
import './wc-app-shell.js';
import type { WcAppShell } from './wc-app-shell.js';
import { dispatchNcToast } from './wc-toast.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-app-shell.preview.js';

async function mount(props: Partial<WcAppShell> = {}): Promise<WcAppShell> {
  const el = document.createElement('wc-app-shell');
  Object.assign(el, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe('wc-app-shell', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders the screen title in the header', async () => {
    const el = await mount({ screenTitle: 'Register' });
    expect(el.shadowRoot?.querySelector('.title')?.textContent).toBe('Register');
  });

  it('exposes the sidebar, header-actions, banner and default slots', async () => {
    const el = await mount();
    const names = [...(el.shadowRoot?.querySelectorAll('slot') ?? [])].map((s) =>
      s.getAttribute('name'),
    );
    expect(names).toContain('sidebar');
    expect(names).toContain('header-actions');
    expect(names).toContain('banner');
    expect(names).toContain(null);
  });

  it('wraps content in a main landmark', async () => {
    const el = await mount();
    expect(el.shadowRoot?.querySelector('main')).toBeTruthy();
  });

  it('hosts exactly one toast region', async () => {
    const el = await mount();
    expect(el.shadowRoot?.querySelectorAll('wc-toast').length).toBe(1);
  });

  it('shows toasts dispatched from its content', async () => {
    const el = await mount();
    const toast = el.shadowRoot?.querySelector('wc-toast');
    await toast?.updateComplete;

    dispatchNcToast(el, { message: 'Saved.' });
    await toast?.updateComplete;

    expect(toast?.shadowRoot?.querySelector('.toast')?.textContent).toContain('Saved.');
  });

  it('reflects sidebar-collapsed so CSS can key off it', async () => {
    const el = await mount({ sidebarCollapsed: true });
    expect(el.hasAttribute('sidebar-collapsed')).toBe(true);
  });

  it('exposes its furniture as parts', async () => {
    const el = await mount();
    // `::part()` is the only way a document stylesheet reaches inside a shadow
    // root, and the print sheet has to hide all three of these to give the page
    // over to the content.
    const parts = [...(el.shadowRoot?.querySelectorAll('[part]') ?? [])].map((node) =>
      node.getAttribute('part'),
    );
    expect(parts).toEqual(['sidebar', 'header', 'banner', 'content']);
  });
});

describePreviewA11y(preview);
