import { describe, it, expect, afterEach, vi } from 'vitest';
import './wc-confirm.js';
import { confirmDialog, type WcConfirm } from './wc-confirm.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-confirm.preview.js';

async function mount(props: Partial<WcConfirm> = {}): Promise<WcConfirm> {
  const el = document.createElement('wc-confirm');
  Object.assign(el, { message: 'Really?' }, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

const click = (el: WcConfirm, sel: string) =>
  el.shadowRoot?.querySelector<HTMLElement>(sel)?.click();

describe('wc-confirm', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders the heading and message', async () => {
    const el = await mount({ heading: 'Undo import?', message: 'Deletes 42 rows.' });
    expect(el.shadowRoot?.querySelector('.message')?.textContent).toBe('Deletes 42 rows.');
    expect(el.shadowRoot?.querySelector('wa-dialog')?.getAttribute('label')).toBe(
      'Undo import?',
    );
  });

  it('emits nc-confirm and closes on confirm', async () => {
    const el = await mount({ open: true });
    const spy = vi.fn();
    el.addEventListener('nc-confirm', spy);

    click(el, '[data-confirm]');
    await el.updateComplete;

    expect(spy).toHaveBeenCalledOnce();
    expect(el.open).toBe(false);
  });

  it('emits nc-cancel and closes on cancel', async () => {
    const el = await mount({ open: true });
    const spy = vi.fn();
    el.addEventListener('nc-cancel', spy);

    click(el, '[data-cancel]');
    await el.updateComplete;

    expect(spy).toHaveBeenCalledOnce();
    expect(el.open).toBe(false);
  });

  it('treats a dialog dismissal as a cancel', async () => {
    const el = await mount({ open: true });
    const spy = vi.fn();
    el.addEventListener('nc-cancel', spy);

    const dialog = el.shadowRoot?.querySelector('wa-dialog');
    dialog?.dispatchEvent(new CustomEvent('wa-hide'));
    await el.updateComplete;

    expect(spy).toHaveBeenCalledOnce();
    expect(el.open).toBe(false);
  });

  it('show() opens and focuses the safe choice', async () => {
    const el = await mount();
    el.show();
    await el.updateComplete;
    await el.updateComplete;

    expect(el.open).toBe(true);
    expect(el.shadowRoot?.activeElement).toBe(
      el.shadowRoot?.querySelector('[data-cancel]'),
    );
  });

  it('uses the danger variant on the confirm button', async () => {
    const el = await mount({ open: true, variant: 'danger' });
    expect(el.shadowRoot?.querySelector('[data-confirm]')?.getAttribute('variant')).toBe(
      'danger',
    );
  });

  describe('confirmDialog', () => {
    it('resolves true when confirmed and cleans up', async () => {
      const promise = confirmDialog({ message: 'Proceed?' });
      const el = document.querySelector('wc-confirm') as WcConfirm;
      await el.updateComplete;

      click(el, '[data-confirm]');

      await expect(promise).resolves.toBe(true);
      expect(document.querySelector('wc-confirm')).toBeNull();
    });

    it('resolves false when cancelled and cleans up', async () => {
      const promise = confirmDialog({ message: 'Proceed?' });
      const el = document.querySelector('wc-confirm') as WcConfirm;
      await el.updateComplete;

      click(el, '[data-cancel]');

      await expect(promise).resolves.toBe(false);
      expect(document.querySelector('wc-confirm')).toBeNull();
    });

    it('passes the options through', async () => {
      const promise = confirmDialog({
        message: 'Delete?',
        heading: 'Careful',
        confirmLabel: 'Delete it',
        variant: 'danger',
      });
      const el = document.querySelector('wc-confirm') as WcConfirm;
      await el.updateComplete;

      expect(el.heading).toBe('Careful');
      expect(el.confirmLabel).toBe('Delete it');
      expect(el.variant).toBe('danger');

      click(el, '[data-cancel]');
      await promise;
    });
  });
});

describePreviewA11y(preview);
