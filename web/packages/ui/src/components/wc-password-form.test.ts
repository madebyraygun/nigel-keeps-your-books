import { describe, it, expect, afterEach } from 'vitest';
import './wc-password-form.js';
import type { WcPasswordForm, NcPasswordSubmitDetail } from './wc-password-form.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-password-form.preview.js';

async function mount(props: Partial<WcPasswordForm> = {}): Promise<WcPasswordForm> {
  const el = document.createElement('wc-password-form');
  Object.assign(el, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

function field(el: WcPasswordForm, name: string): HTMLElement & { value: string } {
  const input = el.shadowRoot?.querySelector(`[data-${name}]`);
  if (!input) throw new Error(`no ${name} field`);
  return input as HTMLElement & { value: string };
}

function submit(el: WcPasswordForm): NcPasswordSubmitDetail[] {
  const seen: NcPasswordSubmitDetail[] = [];
  el.addEventListener('nc-password-submit', (e) =>
    seen.push((e as CustomEvent<NcPasswordSubmitDetail>).detail),
  );
  el.shadowRoot
    ?.querySelector('form')
    ?.dispatchEvent(new Event('submit', { cancelable: true, bubbles: false }));
  return seen;
}

describe('wc-password-form', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders only the fields its mode collects', async () => {
    const set = await mount({ mode: 'set' });
    expect(set.shadowRoot?.querySelector('[data-current]')).toBeNull();
    expect(set.shadowRoot?.querySelector('[data-new]')).toBeTruthy();

    const remove = await mount({ mode: 'remove' });
    expect(remove.shadowRoot?.querySelector('[data-current]')).toBeTruthy();
    expect(remove.shadowRoot?.querySelector('[data-new]')).toBeNull();
  });

  it('submits the new password once, without the confirmation', async () => {
    // The confirmation exists to catch a typo; the server has no use for it.
    const el = await mount({ mode: 'set' });
    field(el, 'new').value = 'hunter2';
    field(el, 'confirm').value = 'hunter2';

    expect(submit(el)).toEqual([{ mode: 'set', newPassword: 'hunter2' }]);
  });

  it('submits both passwords when changing', async () => {
    const el = await mount({ mode: 'change' });
    field(el, 'current').value = 'old one';
    field(el, 'new').value = 'new one';
    field(el, 'confirm').value = 'new one';

    expect(submit(el)).toEqual([
      { mode: 'change', currentPassword: 'old one', newPassword: 'new one' },
    ]);
  });

  it('refuses a mismatch without telling the server', async () => {
    const el = await mount({ mode: 'set' });
    field(el, 'new').value = 'hunter2';
    field(el, 'confirm').value = 'hunter3';

    expect(submit(el)).toEqual([]);
    await el.updateComplete;
    expect(el.shadowRoot?.querySelector('.message')?.textContent).toContain(
      'Passwords do not match.',
    );
  });

  it('refuses an empty new password', async () => {
    const el = await mount({ mode: 'set' });
    field(el, 'new').value = '   ';
    field(el, 'confirm').value = '   ';

    expect(submit(el)).toEqual([]);
    await el.updateComplete;
    expect(el.shadowRoot?.querySelector('.message')?.textContent).toContain(
      'Password cannot be empty.',
    );
  });

  it('refuses a missing current password', async () => {
    const el = await mount({ mode: 'remove' });
    expect(submit(el)).toEqual([]);
    await el.updateComplete;
    expect(el.shadowRoot?.querySelector('.message')?.textContent).toContain(
      'Enter the current password.',
    );
  });

  it('clears every field after a successful submit', async () => {
    const el = await mount({ mode: 'change' });
    field(el, 'current').value = 'old one';
    field(el, 'new').value = 'new one';
    field(el, 'confirm').value = 'new one';
    submit(el);

    expect(field(el, 'current').value).toBe('');
    expect(field(el, 'new').value).toBe('');
    expect(field(el, 'confirm').value).toBe('');
  });

  it('shows a server error', async () => {
    const el = await mount({ mode: 'change', error: 'Wrong password.' });
    expect(el.shadowRoot?.querySelector('.message')?.textContent).toContain(
      'Wrong password.',
    );
  });

  it('does not submit while busy', async () => {
    const el = await mount({ mode: 'set', busy: true });
    field(el, 'new').value = 'hunter2';
    field(el, 'confirm').value = 'hunter2';
    expect(submit(el)).toEqual([]);
  });
});

describePreviewA11y(preview);
