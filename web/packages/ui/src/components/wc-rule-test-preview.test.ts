import { describe, it, expect, afterEach } from 'vitest';
import './wc-rule-test-preview.js';
import type { WcRuleTestPreview } from './wc-rule-test-preview.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-rule-test-preview.preview.js';

async function mount(
  props: Partial<WcRuleTestPreview> = {},
): Promise<WcRuleTestPreview> {
  const el = document.createElement('wc-rule-test-preview');
  Object.assign(el, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function text(el: WcRuleTestPreview): string {
  return el.shadowRoot?.textContent?.replace(/\s+/g, ' ').trim() ?? '';
}

describe('wc-rule-test-preview', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('announces its updates politely', async () => {
    const el = await mount();
    expect(el.shadowRoot?.querySelector('[aria-live="polite"]')).not.toBeNull();
  });

  it('invites a pattern before anything has been tested', async () => {
    const el = await mount();
    expect(text(el)).toContain('Type a pattern');
  });

  it('shows a spinner while testing', async () => {
    const el = await mount({ busy: true });
    expect(el.shadowRoot?.querySelector('wc-spinner')).not.toBeNull();
  });

  it('lists matches with their counts', async () => {
    const el = await mount({
      result: {
        total: 5,
        matches: [
          { description: 'ADOBE CREATIVE CLOUD', count: 3 },
          { description: 'ADOBE *STOCK', count: 2 },
        ],
      },
    });
    expect(text(el)).toContain('Matches 5 transactions');
    const items = [...(el.shadowRoot?.querySelectorAll('li') ?? [])].map((n) =>
      n.textContent?.replace(/\s+/g, ' ').trim(),
    );
    expect(items).toEqual(['ADOBE CREATIVE CLOUD ×3', 'ADOBE *STOCK ×2']);
  });

  it('drops the multiplier for a description that appears once', async () => {
    const el = await mount({
      result: { total: 1, matches: [{ description: 'RENT MARCH 2025', count: 1 }] },
    });
    expect(text(el)).toContain('Matches 1 transaction:');
    expect(text(el)).not.toContain('×');
  });

  it('treats no matches as an answer, not a failure', async () => {
    const el = await mount({ result: { total: 0, matches: [] } });
    expect(text(el)).toContain('Nothing matches this pattern yet');
    expect(el.shadowRoot?.querySelector('.error')).toBeNull();
  });

  it('renders a rejected pattern as an error', async () => {
    const el = await mount({ error: 'Invalid regex: unclosed group' });
    expect(el.shadowRoot?.querySelector('.error')?.textContent).toContain(
      'unclosed group',
    );
  });

  it('prefers the error over a stale result', async () => {
    const el = await mount({
      error: 'Invalid regex: unclosed group',
      result: { total: 9, matches: [{ description: 'STALE', count: 9 }] },
    });
    expect(text(el)).not.toContain('STALE');
  });
});

describePreviewA11y(preview);
