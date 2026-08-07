import { describe, it, expect, afterEach } from 'vitest';
import './wc-review-progress.js';
import type { WcReviewProgress } from './wc-review-progress.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-review-progress.preview.js';

async function mount(props: Partial<WcReviewProgress> = {}): Promise<WcReviewProgress> {
  const el = document.createElement('wc-review-progress');
  Object.assign(el, { current: 1, total: 12, ...props });
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function bar(el: WcReviewProgress): Element {
  const found = el.shadowRoot?.querySelector('[role="progressbar"]');
  if (!found) throw new Error('no progressbar');
  return found;
}

describe('wc-review-progress', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('reads "n of m"', async () => {
    const el = await mount({ current: 3, total: 12 });
    expect(el.shadowRoot?.querySelector('.position')?.textContent?.trim()).toBe(
      '3 of 12',
    );
    expect(bar(el).getAttribute('aria-valuetext')).toBe('3 of 12');
  });

  it('counts transactions finished, not the one on screen', async () => {
    // Sitting on #3 of 12 means two are done — a bar that filled to 3 would
    // claim credit for a decision that has not been made yet.
    const el = await mount({ current: 3, total: 12 });
    expect(bar(el).getAttribute('aria-valuenow')).toBe('2');
    expect(bar(el).getAttribute('aria-valuemax')).toBe('12');
  });

  it('is empty at the start and full at the end', async () => {
    const start = await mount({ current: 1, total: 4 });
    expect(
      start.shadowRoot?.querySelector<HTMLElement>('.fill')?.style.width,
    ).toBe('0%');

    document.body.innerHTML = '';
    const end = await mount({ current: 5, total: 4 });
    expect(end.shadowRoot?.querySelector<HTMLElement>('.fill')?.style.width).toBe(
      '100%',
    );
  });

  it('never reports a position past the end of the queue', async () => {
    const el = await mount({ current: 13, total: 12 });
    expect(el.shadowRoot?.querySelector('.position')?.textContent?.trim()).toBe(
      '12 of 12',
    );
  });

  it('shows a tally only once there is something to tally', async () => {
    const none = await mount({ current: 1, total: 12 });
    expect(none.shadowRoot?.querySelector('.tally')).toBeNull();

    document.body.innerHTML = '';
    const some = await mount({ current: 8, total: 12, reviewed: 5, skipped: 2 });
    expect(some.shadowRoot?.querySelector('.tally')?.textContent?.trim()).toBe(
      '5 reviewed · 2 skipped',
    );
  });

  it('survives an empty queue without dividing by zero', async () => {
    const el = await mount({ current: 1, total: 0 });
    expect(el.shadowRoot?.querySelector<HTMLElement>('.fill')?.style.width).toBe('0%');
  });
});

describePreviewA11y(preview);
