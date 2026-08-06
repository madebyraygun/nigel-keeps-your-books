import { describe, it, expect, afterEach, vi } from 'vitest';
import './wc-toast.js';
import { dispatchNcToast, type WcToast } from './wc-toast.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-toast.preview.js';

async function mount(): Promise<WcToast> {
  const el = document.createElement('wc-toast');
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function toastEl(el: WcToast): HTMLElement | null {
  return el.shadowRoot?.querySelector('.toast') ?? null;
}

function region(el: WcToast): HTMLElement | null {
  return el.shadowRoot?.querySelector('[data-toast-region]') ?? null;
}

describe('wc-toast', () => {
  afterEach(() => {
    document.body.innerHTML = '';
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it('starts empty but keeps the live region mounted', async () => {
    const el = await mount();
    expect(toastEl(el)).toBeNull();
    // The region has to persist or assistive tech never subscribes to it.
    expect(region(el)).toBeTruthy();
  });

  it('shows a toast dispatched from an unrelated element', async () => {
    const el = await mount();
    // Deliberately not a descendant: the bus listens on window precisely so
    // detached and top-layer subtrees still reach the region.
    const stranger = document.createElement('div');
    document.body.appendChild(stranger);

    dispatchNcToast(stranger, { message: 'Saved.' });
    await el.updateComplete;

    expect(toastEl(el)?.textContent).toContain('Saved.');
  });

  it('announces politely for info and success', async () => {
    const el = await mount();
    dispatchNcToast(el, { message: 'Done.', variant: 'success' });
    await el.updateComplete;
    expect(region(el)?.getAttribute('role')).toBe('status');
    expect(region(el)?.getAttribute('aria-live')).toBe('polite');
  });

  it('announces assertively for danger', async () => {
    const el = await mount();
    dispatchNcToast(el, { message: 'Failed.', variant: 'danger' });
    await el.updateComplete;
    expect(region(el)?.getAttribute('role')).toBe('alert');
    expect(region(el)?.getAttribute('aria-live')).toBe('assertive');
  });

  it('auto-dismisses after the default duration', async () => {
    vi.useFakeTimers();
    const el = await mount();
    dispatchNcToast(el, { message: 'Transient.' });
    await el.updateComplete;
    expect(toastEl(el)).toBeTruthy();

    vi.advanceTimersByTime(4000);
    await el.updateComplete;
    expect(toastEl(el)).toBeNull();
  });

  it('gives an actionable toast longer to be read', async () => {
    vi.useFakeTimers();
    const el = await mount();
    dispatchNcToast(el, { message: 'Undone.', action: { label: 'Redo', onClick: () => {} } });
    await el.updateComplete;

    vi.advanceTimersByTime(4000);
    await el.updateComplete;
    expect(toastEl(el)).toBeTruthy();

    vi.advanceTimersByTime(4000);
    await el.updateComplete;
    expect(toastEl(el)).toBeNull();
  });

  it('stays put when the duration is zero', async () => {
    vi.useFakeTimers();
    const el = await mount();
    dispatchNcToast(el, { message: 'Sticky.', duration: 0 });
    await el.updateComplete;

    vi.advanceTimersByTime(60_000);
    await el.updateComplete;
    expect(toastEl(el)).toBeTruthy();
  });

  it('invokes the action and dismisses on click', async () => {
    const onClick = vi.fn();
    const el = await mount();
    dispatchNcToast(el, { message: 'Undone.', action: { label: 'Redo', onClick } });
    await el.updateComplete;

    el.shadowRoot?.querySelector<HTMLButtonElement>('[data-toast-action]')?.click();
    await el.updateComplete;

    expect(onClick).toHaveBeenCalledOnce();
    expect(toastEl(el)).toBeNull();
  });

  it('survives an action that throws', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    const el = await mount();
    dispatchNcToast(el, {
      message: 'Boom.',
      action: {
        label: 'Explode',
        onClick: () => {
          throw new Error('nope');
        },
      },
    });
    await el.updateComplete;

    expect(() =>
      el.shadowRoot?.querySelector<HTMLButtonElement>('[data-toast-action]')?.click(),
    ).not.toThrow();
    await el.updateComplete;
    expect(toastEl(el)).toBeNull();
  });

  it('ignores an event with no message', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    const el = await mount();
    el.dispatchEvent(
      new CustomEvent('nc-toast', { detail: { message: '' }, bubbles: true, composed: true }),
    );
    await el.updateComplete;
    expect(toastEl(el)).toBeNull();
  });

  it('stops listening once disconnected', async () => {
    const el = await mount();
    el.remove();
    dispatchNcToast(document.body, { message: 'Too late.' });
    await el.updateComplete;
    expect(toastEl(el)).toBeNull();
  });

  it('seeds a toast from .initial', async () => {
    const el = document.createElement('wc-toast');
    el.initial = { message: 'Seeded.', duration: 0 };
    document.body.appendChild(el);
    await el.updateComplete;
    expect(toastEl(el)?.textContent).toContain('Seeded.');
  });
});

describePreviewA11y(preview);
