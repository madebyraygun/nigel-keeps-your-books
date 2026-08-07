import { describe, it, expect, afterEach } from 'vitest';
import './wc-manager-dialog.js';
import type { WcManagerDialog } from './wc-manager-dialog.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-manager-dialog.preview.js';

async function mount(props: Partial<WcManagerDialog> = {}): Promise<WcManagerDialog> {
  const el = document.createElement('wc-manager-dialog');
  Object.assign(el, { heading: 'Add account', open: true }, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe('wc-manager-dialog', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('emits nc-manager-save from the save button', async () => {
    const el = await mount();
    let fired = 0;
    el.addEventListener('nc-manager-save', () => (fired += 1));
    el.shadowRoot?.querySelector<HTMLElement>('[data-save]')?.click();
    expect(fired).toBe(1);
  });

  it('emits nc-manager-cancel from the cancel button', async () => {
    const el = await mount();
    let fired = 0;
    el.addEventListener('nc-manager-cancel', () => (fired += 1));
    el.shadowRoot?.querySelector<HTMLElement>('[data-cancel]')?.click();
    expect(fired).toBe(1);
  });

  it("treats wa-dialog's own dismissal as a cancel", async () => {
    const el = await mount();
    let fired = 0;
    el.addEventListener('nc-manager-cancel', () => (fired += 1));
    el.shadowRoot
      ?.querySelector('wa-dialog')
      ?.dispatchEvent(new CustomEvent('wa-hide'));
    expect(fired).toBe(1);
  });

  it('ignores a wa-hide re-targeted from something inside the dialog', async () => {
    // wa-dialog re-emits some inner events; reacting to those would close the
    // form under a user who only dismissed a select.
    const el = await mount();
    let fired = 0;
    el.addEventListener('nc-manager-cancel', () => (fired += 1));

    const inner = document.createElement('div');
    el.shadowRoot?.querySelector('wa-dialog')?.appendChild(inner);
    inner.dispatchEvent(new CustomEvent('wa-hide', { bubbles: true }));

    expect(fired).toBe(0);
  });

  it('renders the server message in an alert region', async () => {
    const el = await mount({ error: 'An account named “BofA Checking” already exists.' });
    const alert = el.shadowRoot?.querySelector('.error[role="alert"]');
    expect(alert?.textContent).toContain('already exists');
  });

  it('will not save twice while a save is in flight', async () => {
    const el = await mount({ busy: true });
    let fired = 0;
    el.addEventListener('nc-manager-save', () => (fired += 1));

    const save = el.shadowRoot?.querySelector<HTMLElement>('[data-save]');
    expect(save?.hasAttribute('disabled')).toBe(true);
    save?.click();
    expect(fired).toBe(0);
    expect(save?.textContent?.trim()).toBe('Saving…');
  });
});

describePreviewA11y(preview);
