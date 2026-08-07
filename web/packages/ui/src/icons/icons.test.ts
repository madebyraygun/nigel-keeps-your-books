import { describe, it, expect, beforeAll } from 'vitest';
import './icons.js';
import { ICON_TAGS } from './icons.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './icons.preview.js';

async function mount(tag: string, label?: string): Promise<HTMLElement> {
  const el = document.createElement(tag);
  if (label !== undefined) el.setAttribute('label', label);
  document.body.appendChild(el);
  await (el as HTMLElement & { updateComplete: Promise<unknown> }).updateComplete;
  return el;
}

describe('icons', () => {
  beforeAll(() => {
    document.body.innerHTML = '';
  });

  it.each(ICON_TAGS)('%s is registered', (tag) => {
    expect(customElements.get(tag)).toBeDefined();
  });

  it.each(ICON_TAGS)('%s renders an svg with geometry', async (tag) => {
    const el = await mount(tag);
    const svg = el.shadowRoot?.querySelector('svg');
    expect(svg).toBeTruthy();
    expect(svg?.querySelectorAll('path').length).toBeGreaterThan(0);
    el.remove();
  });

  it('is hidden from assistive tech when unlabelled', async () => {
    const el = await mount('wc-icon-flag');
    const svg = el.shadowRoot?.querySelector('svg');
    expect(svg?.getAttribute('role')).toBe('presentation');
    expect(svg?.getAttribute('aria-hidden')).toBe('true');
    el.remove();
  });

  it('becomes an img with an accessible name when labelled', async () => {
    const el = await mount('wc-icon-flag', 'Flagged');
    const svg = el.shadowRoot?.querySelector('svg');
    expect(svg?.getAttribute('role')).toBe('img');
    expect(svg?.getAttribute('aria-hidden')).toBe('false');
    expect(svg?.getAttribute('aria-label')).toBe('Flagged');
    el.remove();
  });
});

describePreviewA11y(preview);
