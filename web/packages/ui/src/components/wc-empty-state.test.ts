import { describe, it, expect, afterEach } from 'vitest';
import './wc-empty-state.js';
import type { WcEmptyState } from './wc-empty-state.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-empty-state.preview.js';

async function mount(props: Partial<WcEmptyState> = {}): Promise<WcEmptyState> {
  const el = document.createElement('wc-empty-state');
  Object.assign(el, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe('wc-empty-state', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders the heading and message', async () => {
    const el = await mount({ heading: 'No rows', message: 'Try another filter.' });
    expect(el.shadowRoot?.querySelector('.heading')?.textContent).toBe('No rows');
    expect(el.shadowRoot?.querySelector('.message')?.textContent).toBe(
      'Try another filter.',
    );
  });

  it('omits the heading when none is given', async () => {
    const el = await mount({ message: 'Nothing here.' });
    expect(el.shadowRoot?.querySelector('.heading')).toBeNull();
  });

  it('renders the named icon element', async () => {
    const el = await mount({ icon: 'wc-icon-register', message: 'Empty' });
    expect(el.shadowRoot?.querySelector('wc-icon-register')).toBeTruthy();
  });

  it('renders no icon element when the icon is unset', async () => {
    const el = await mount({ message: 'Empty' });
    expect(el.shadowRoot?.querySelector('.icon')).toBeNull();
  });

  it('exposes an actions slot', async () => {
    const el = await mount({ message: 'Empty' });
    expect(el.shadowRoot?.querySelector('slot[name="actions"]')).toBeTruthy();
  });
});

describePreviewA11y(preview);
