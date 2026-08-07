import { describe, it, expect, afterEach } from 'vitest';
import './wc-manager-layout.js';
import type { WcManagerLayout } from './wc-manager-layout.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-manager-layout.preview.js';

async function mount(props: Partial<WcManagerLayout> = {}): Promise<WcManagerLayout> {
  const el = document.createElement('wc-manager-layout');
  Object.assign(el, { heading: 'Accounts' }, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function shadow(el: WcManagerLayout, selector: string): HTMLElement | null {
  return el.shadowRoot?.querySelector<HTMLElement>(selector) ?? null;
}

describe('wc-manager-layout', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders the heading and, when set, the count beside it', async () => {
    const el = await mount({ count: 12 });
    const heading = shadow(el, '.heading');
    expect(heading?.textContent).toContain('Accounts');
    expect(heading?.textContent).toContain('(12)');
  });

  it('omits the count when it is null', async () => {
    const el = await mount();
    expect(shadow(el, '.count')).toBeNull();
  });

  it('emits nc-manager-add when Add is pressed', async () => {
    const el = await mount();
    let fired = 0;
    el.addEventListener('nc-manager-add', () => (fired += 1));
    shadow(el, '[data-add]')?.click();
    expect(fired).toBe(1);
  });

  it('disables Add when asked', async () => {
    const el = await mount({ addDisabled: true });
    expect(shadow(el, '[data-add]')?.hasAttribute('disabled')).toBe(true);
  });

  it('renders a guardrail in an alert region, not a quiet line', async () => {
    // AC #4: a refused delete has to be announced, and has to still be on
    // screen when the user goes looking for the number in it.
    const el = await mount({
      error: '5 transactions use this category. Recategorize them first.',
    });
    const alert = shadow(el, '.error[role="alert"]');
    expect(alert?.textContent).toContain('5 transactions use this category');
  });

  it('offers the error action only when it is labelled, and emits it', async () => {
    const plain = await mount({ error: 'Nope.' });
    expect(shadow(plain, '[data-error-action]')).toBeNull();

    const withAction = await mount({
      error: '3 active rules assign this category.',
      errorActionLabel: 'Show those rules',
    });
    let fired = 0;
    withAction.addEventListener('nc-manager-error-action', () => (fired += 1));
    shadow(withAction, '[data-error-action]')?.click();
    expect(fired).toBe(1);
  });

  it('emits nc-manager-error-dismiss from the dismiss button', async () => {
    const el = await mount({ error: 'Nope.' });
    let fired = 0;
    el.addEventListener('nc-manager-error-dismiss', () => (fired += 1));
    shadow(el, '[data-error-dismiss]')?.click();
    expect(fired).toBe(1);
  });

  it('swaps the list slot for the empty slot', async () => {
    const list = await mount();
    expect(list.shadowRoot?.querySelector('slot:not([name])')).toBeTruthy();
    expect(list.shadowRoot?.querySelector('slot[name="empty"]')).toBeNull();

    const empty = await mount({ empty: true });
    expect(empty.shadowRoot?.querySelector('slot[name="empty"]')).toBeTruthy();
    expect(empty.shadowRoot?.querySelector('slot:not([name])')).toBeNull();
  });

  it('shows a spinner instead of the list while busy', async () => {
    const el = await mount({ busy: true });
    expect(shadow(el, 'wc-spinner')).toBeTruthy();
    expect(el.shadowRoot?.querySelector('slot:not([name])')).toBeNull();
  });

  it('always keeps the overlay slot, so a dialog survives a busy reload', async () => {
    const el = await mount({ busy: true });
    expect(el.shadowRoot?.querySelector('slot[name="overlay"]')).toBeTruthy();
  });
});

describePreviewA11y(preview);
