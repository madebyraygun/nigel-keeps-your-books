import { describe, it, expect, afterEach } from 'vitest';
import './wc-unlock-card.js';
import type { WcUnlockCard, NcUnlockDetail } from './wc-unlock-card.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-unlock-card.preview.js';

async function mount(props: Partial<WcUnlockCard> = {}): Promise<WcUnlockCard> {
  const el = document.createElement('wc-unlock-card');
  Object.assign(el, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function passwordField(el: WcUnlockCard): HTMLElement & { value: string } {
  const input = el.shadowRoot?.querySelector('wa-input');
  if (!input) throw new Error('no password field');
  return input as HTMLElement & { value: string };
}

function submit(el: WcUnlockCard): void {
  el.shadowRoot?.querySelector('form')?.dispatchEvent(
    new Event('submit', { cancelable: true, bubbles: false }),
  );
}

describe('wc-unlock-card', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('emits the typed password on submit', async () => {
    const el = await mount();
    const seen: NcUnlockDetail[] = [];
    el.addEventListener('nc-unlock', (e) => seen.push((e as CustomEvent<NcUnlockDetail>).detail));

    passwordField(el).value = 'hunter2';
    submit(el);

    expect(seen).toEqual([{ password: 'hunter2' }]);
  });

  it('clears the field once the password is on its way', async () => {
    // A failed attempt should not leave the password sitting in the DOM.
    const el = await mount();
    passwordField(el).value = 'hunter2';
    submit(el);
    expect(passwordField(el).value).toBe('');
  });

  it('never reflects the password to an attribute', async () => {
    const el = await mount();
    passwordField(el).value = 'hunter2';
    submit(el);
    expect(el.outerHTML).not.toContain('hunter2');
  });

  it('does not submit an empty password', async () => {
    const el = await mount();
    let fired = 0;
    el.addEventListener('nc-unlock', () => (fired += 1));
    submit(el);
    expect(fired).toBe(0);
  });

  it('does not submit while a request is in flight', async () => {
    const el = await mount({ busy: true });
    let fired = 0;
    el.addEventListener('nc-unlock', () => (fired += 1));
    passwordField(el).value = 'hunter2';
    submit(el);
    expect(fired).toBe(0);
  });

  it('shows the error and how many attempts are left', async () => {
    const el = await mount({ error: 'Wrong password.', attemptsRemaining: 2 });
    const status = el.shadowRoot?.querySelector('.status')?.textContent ?? '';
    expect(status).toContain('Wrong password.');
    expect(status).toContain('2 attempts remaining');
  });

  it('says attempt, singular, when one is left', async () => {
    const el = await mount({ error: 'Wrong password.', attemptsRemaining: 1 });
    expect(el.shadowRoot?.querySelector('.attempts')?.textContent).toContain(
      '1 attempt remaining',
    );
  });

  it('claims nothing about attempts when the server did not say', async () => {
    const el = await mount({ error: 'Could not reach the nigel server.' });
    expect(el.shadowRoot?.querySelector('.attempts')).toBeNull();
  });

  it('counts down the throttle while the request is in flight', async () => {
    // The server serves the delay before answering, so the countdown belongs to
    // the in-flight request rather than to a cooldown after it.
    const el = await mount({ busy: true, countdownSeconds: 4 });
    expect(el.shadowRoot?.querySelector('.countdown')?.textContent).toContain(
      'checking in 4s',
    );
  });

  it('announces status changes politely', async () => {
    const el = await mount({ error: 'Wrong password.' });
    const status = el.shadowRoot?.querySelector('.status');
    expect(status?.getAttribute('aria-live')).toBe('polite');
  });
});

describePreviewA11y(preview);
