import { describe, it, expect, afterEach } from 'vitest';
import './wc-panel.js';
import type { WcPanel } from './wc-panel.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-panel.preview.js';

async function mount(props: Partial<WcPanel> = {}): Promise<WcPanel> {
  const el = document.createElement('wc-panel');
  Object.assign(el, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe('wc-panel', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders the heading and description', async () => {
    const el = await mount({ heading: 'Business name', description: 'Shown in the sidebar.' });
    expect(el.shadowRoot?.querySelector('.heading')?.textContent).toBe('Business name');
    expect(el.shadowRoot?.querySelector('.description')?.textContent).toBe(
      'Shown in the sidebar.',
    );
  });

  it('omits the heading and description when unset', async () => {
    const el = await mount();
    expect(el.shadowRoot?.querySelector('.heading')).toBeNull();
    expect(el.shadowRoot?.querySelector('.description')).toBeNull();
  });

  it('exposes a body slot and an actions slot', async () => {
    const el = await mount({ heading: 'Panel' });
    expect(el.shadowRoot?.querySelector('.body slot:not([name])')).toBeTruthy();
    expect(el.shadowRoot?.querySelector('slot[name="actions"]')).toBeTruthy();
  });

  it('titles the section with a heading element, not a styled div', async () => {
    // The settings screen is a stack of these; screen-reader users navigate it
    // by heading.
    const el = await mount({ heading: 'Database password' });
    expect(el.shadowRoot?.querySelector('h2.heading')).toBeTruthy();
  });
});

describePreviewA11y(preview);
