import { describe, it, expect, afterEach } from 'vitest';
import './wc-send-dialog.js';
import type { SendStepView, WcSendDialog } from './wc-send-dialog.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-send-dialog.preview.js';

const STEPS: SendStepView[] = [
  { step: 'config', label: 'Reading the invoicing settings', state: 'ok' },
  { step: 'payment_link', label: 'Creating the Stripe payment link', state: 'reused' },
  { step: 'publish', label: 'Publishing to R2', state: 'failed' },
  { step: 'email', label: 'Emailing the client', state: 'pending' },
];

async function mount(props: Partial<WcSendDialog> = {}): Promise<WcSendDialog> {
  const el = document.createElement('wc-send-dialog');
  Object.assign(
    el,
    { open: true, number: 1251, total: 1850, recipient: 'ap@acme.test' },
    props,
  );
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe('wc-send-dialog', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('names every consequence before anything happens', async () => {
    const el = await mount({ publishHost: 'billing.rygn.io' });
    const consequences = el.shadowRoot?.querySelector('[data-consequences]');
    expect(consequences?.textContent).toContain('Stripe payment link');
    expect(consequences?.textContent).toContain('billing.rygn.io');
    expect(consequences?.textContent).toContain('ap@acme.test');
    expect(
      (el.shadowRoot?.querySelector('wc-money') as HTMLElement & { amount: number }).amount,
    ).toBe(1850);
  });

  it('shows the subject the email will carry', async () => {
    const el = await mount({ subject: 'Invoice #1251 from Raygun' });
    expect(el.shadowRoot?.querySelector('[data-subject]')?.textContent).toContain(
      'Invoice #1251 from Raygun',
    );
  });

  it('asks for confirmation and emits only when it is given', async () => {
    const el = await mount();
    let confirms = 0;
    el.addEventListener('nc-send-confirm', () => (confirms += 1));

    el.shadowRoot?.querySelector<HTMLElement>('[data-confirm]')?.click();
    expect(confirms).toBe(1);
  });

  it('refuses to confirm at all when the invoice cannot be sent', async () => {
    const el = await mount({ blocked: 'Globex has no email address.' });
    let confirms = 0;
    el.addEventListener('nc-send-confirm', () => (confirms += 1));

    const button = el.shadowRoot?.querySelector('[data-confirm]');
    expect(button?.hasAttribute('disabled')).toBe(true);
    (button as HTMLElement).click();
    expect(confirms).toBe(0);
    expect(el.shadowRoot?.querySelector('[data-blocked]')?.textContent).toContain(
      'no email address',
    );
  });

  it('renders the step trace with a state per step', async () => {
    const el = await mount({ phase: 'failed', steps: STEPS });
    const items = [...(el.shadowRoot?.querySelectorAll('[data-step]') ?? [])].map((li) => [
      li.getAttribute('data-step'),
      li.getAttribute('data-state'),
    ]);
    expect(items).toEqual([
      ['config', 'ok'],
      ['payment_link', 'reused'],
      ['publish', 'failed'],
      ['email', 'pending'],
    ]);
  });

  it('gives each step a word beside its glyph', async () => {
    // The glyph is decoration; "✗" announces as nothing useful.
    const el = await mount({ phase: 'failed', steps: STEPS });
    const first = el.shadowRoot?.querySelector('[data-step="config"]');
    expect(first?.querySelector('.glyph')?.getAttribute('aria-hidden')).toBe('true');
    expect(first?.querySelector('.sr-only')?.textContent).toContain('done');
  });

  it('shows the upstream sentence verbatim under our own headline', async () => {
    const el = await mount({
      phase: 'failed',
      steps: STEPS,
      failure: {
        headline: 'Publishing the invoice page failed.',
        message: 'r2 403: SignatureDoesNotMatch',
        note: 'No email was sent.',
        retryable: true,
      },
    });
    expect(el.shadowRoot?.querySelector('[data-failure] h3')?.textContent).toBe(
      'Publishing the invoice page failed.',
    );
    expect(el.shadowRoot?.querySelector('[data-upstream]')?.textContent).toBe(
      'r2 403: SignatureDoesNotMatch',
    );
    expect(el.shadowRoot?.querySelector('[data-note]')?.textContent).toContain(
      'No email was sent',
    );
  });

  it('points at the fix when the failure has one', async () => {
    const el = await mount({
      phase: 'failed',
      failure: {
        headline: 'Sending is not configured yet.',
        message: 'Sending needs r2_bucket, which is not set.',
        retryable: false,
        actionLabel: 'Open settings',
        actionHref: '#/settings',
      },
    });
    const link = el.shadowRoot?.querySelector('[data-failure-action]');
    expect(link?.getAttribute('href')).toBe('#/settings');
    expect(link?.textContent).toBe('Open settings');
  });

  it('offers Try again for a retryable failure and none for a record failure', async () => {
    const retryable = await mount({
      phase: 'failed',
      steps: STEPS,
      failure: { headline: 'x', message: 'y', retryable: true },
    });
    expect(retryable.shadowRoot?.querySelector('[data-retry]')).toBeTruthy();

    const notRetryable = await mount({
      phase: 'failed',
      steps: STEPS,
      failure: { headline: 'x', message: 'y', retryable: false },
    });
    expect(notRetryable.shadowRoot?.querySelector('[data-retry]')).toBeNull();
  });

  it('stays open across its own request and cannot be dismissed mid-flight', async () => {
    // `wa-hide` is a request, not a notification: `requestClose` closes unless
    // the event is prevented. Escape, the backdrop and the built-in close
    // button all go through it, so declining to answer would hide the dialog
    // while `open` stayed true — and the send would finish into a dialog the
    // `?open` binding never reopens.
    const el = await mount({ phase: 'sending', steps: STEPS });
    let closes = 0;
    el.addEventListener('nc-send-close', () => (closes += 1));

    expect(el.shadowRoot?.querySelector('wc-spinner')).toBeTruthy();
    expect(el.shadowRoot?.querySelector('[data-close]')?.hasAttribute('disabled')).toBe(
      true,
    );

    const dialog = el.shadowRoot?.querySelector('wa-dialog');
    const hide = new CustomEvent('wa-hide', { bubbles: false, cancelable: true });
    dialog?.dispatchEvent(hide);

    expect(hide.defaultPrevented, 'the dismissal must be refused, not ignored').toBe(true);
    expect(closes).toBe(0);
    expect(el.open).toBe(true);
  });

  it('lets a finished send be dismissed by Escape or the backdrop', async () => {
    const el = await mount({ phase: 'failed', steps: STEPS });
    let closes = 0;
    el.addEventListener('nc-send-close', () => (closes += 1));

    const dialog = el.shadowRoot?.querySelector('wa-dialog');
    const hide = new CustomEvent('wa-hide', { bubbles: false, cancelable: true });
    dialog?.dispatchEvent(hide);

    expect(hide.defaultPrevented).toBe(false);
    expect(closes).toBe(1);
  });

  it('closes on Close, which is the only thing that resolves it', async () => {
    const el = await mount({ phase: 'sent', steps: STEPS });
    let closes = 0;
    el.addEventListener('nc-send-close', () => (closes += 1));

    el.shadowRoot?.querySelector<HTMLElement>('[data-close]')?.click();
    expect(closes).toBe(1);
  });

  it('reports where a successful send published to', async () => {
    const el = await mount({
      phase: 'sent',
      steps: STEPS,
      publicUrl: 'https://billing.rygn.io/i/aBc123XyZ/',
    });
    expect(el.shadowRoot?.querySelector('[data-sent]')?.textContent).toContain(
      'ap@acme.test',
    );
    expect(el.shadowRoot?.querySelector('[data-public-url]')?.textContent).toContain(
      'billing.rygn.io',
    );
  });
});

describePreviewA11y(preview);
