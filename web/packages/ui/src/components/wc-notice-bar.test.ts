import { describe, it, expect, afterEach } from 'vitest';
import './wc-notice-bar.js';
import type { WcNoticeBar } from './wc-notice-bar.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-notice-bar.preview.js';

async function mount(props: Partial<WcNoticeBar> = {}): Promise<WcNoticeBar> {
  const el = document.createElement('wc-notice-bar');
  Object.assign(el, { message: 'An update is available.', ...props });
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function buttons(el: WcNoticeBar): HTMLButtonElement[] {
  return [...(el.shadowRoot?.querySelectorAll<HTMLButtonElement>('button') ?? [])];
}

describe('wc-notice-bar', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders its message', async () => {
    const el = await mount({ message: 'Nigel v1.0.2 is available.' });
    expect(el.shadowRoot?.querySelector('.message')?.textContent).toContain(
      'Nigel v1.0.2 is available.',
    );
  });

  it('announces without interrupting', async () => {
    const el = await mount();
    expect(el.shadowRoot?.querySelector('.bar')?.getAttribute('role')).toBe(
      'status',
    );
  });

  it('reflects the variant so CSS can key off it', async () => {
    const el = await mount({ variant: 'warning' });
    expect(el.getAttribute('variant')).toBe('warning');
  });

  it('defaults to info', async () => {
    expect((await mount()).variant).toBe('info');
  });

  it('renders an icon only when asked', async () => {
    expect((await mount()).shadowRoot?.querySelector('.icon')).toBeNull();
    const el = await mount({ icon: 'wc-icon-download' });
    expect(el.shadowRoot?.querySelector('wc-icon-download')).not.toBeNull();
  });

  it('has no buttons by default', async () => {
    expect(buttons(await mount()).length).toBe(0);
  });

  it('fires nc-notice-action from the action button', async () => {
    const el = await mount({ actionLabel: 'Review now' });
    let fired = 0;
    document.body.addEventListener('nc-notice-action', () => (fired += 1));
    const action = buttons(el).find((b) => b.textContent?.includes('Review now'));
    action?.click();
    expect(fired).toBe(1);
  });

  it('fires nc-notice-dismiss from the dismiss button', async () => {
    const el = await mount({ dismissible: true });
    let fired = 0;
    document.body.addEventListener('nc-notice-dismiss', () => (fired += 1));
    el.shadowRoot?.querySelector<HTMLButtonElement>('.dismiss')?.click();
    expect(fired).toBe(1);
  });

  it('gives the icon-only dismiss button an accessible name', async () => {
    const el = await mount({ dismissible: true });
    expect(
      el.shadowRoot?.querySelector('.dismiss')?.getAttribute('aria-label'),
    ).toBe('Dismiss');
  });

  it('renders slotted content alongside the message', async () => {
    const el = await mount({ message: '' });
    expect(el.shadowRoot?.querySelector('slot')).not.toBeNull();
  });
});

describePreviewA11y(preview);
