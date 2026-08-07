import { describe, it, expect, afterEach } from 'vitest';
import './wc-spinner.js';
import type { WcSpinner } from './wc-spinner.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-spinner.preview.js';

async function mount(props: Partial<WcSpinner> = {}): Promise<WcSpinner> {
  const el = document.createElement('wc-spinner');
  Object.assign(el, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe('wc-spinner', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('announces itself as a polite status', async () => {
    const el = await mount();
    const status = el.shadowRoot?.querySelector('[role="status"]');
    expect(status).toBeTruthy();
    expect(status?.getAttribute('aria-live')).toBe('polite');
  });

  it('always carries an accessible label, even when not shown', async () => {
    const el = await mount();
    const status = el.shadowRoot?.querySelector('[role="status"]');
    expect(status?.textContent?.trim()).toBe('Loading');
    expect(status?.classList.contains('visually-hidden')).toBe(true);
  });

  it('renders the label visibly when asked', async () => {
    const el = await mount({ showLabel: true, label: 'Connecting' });
    const status = el.shadowRoot?.querySelector('[role="status"]');
    expect(status?.classList.contains('visually-hidden')).toBe(false);
    expect(status?.textContent?.trim()).toBe('Connecting');
  });

  it('reflects size so CSS can key off it', async () => {
    const el = await mount({ size: 'l' });
    expect(el.getAttribute('size')).toBe('l');
  });

  it('stops animating under a reduced-motion preference', async () => {
    const el = await mount();
    const styles = (el.constructor as typeof WcSpinner).styles;
    const cssText = Array.isArray(styles)
      ? styles.map((s) => String(s)).join('\n')
      : String(styles);
    expect(cssText).toMatch(/prefers-reduced-motion:\s*reduce/);
  });
});

describePreviewA11y(preview);
