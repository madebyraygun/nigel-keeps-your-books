import { describe, it, expect, afterEach } from 'vitest';
import './wc-client-form.js';
import {
  EMPTY_CLIENT_FORM,
  validateClientForm,
  type ClientFormValue,
  type NcClientFormChangeDetail,
  type WcClientForm,
} from './wc-client-form.js';
import { describePreviewA11y } from '../../preview/axe-suite.js';
import preview from './wc-client-form.preview.js';

const filled: ClientFormValue = {
  name: 'Acme Co',
  email: 'ap@acme.test',
  billingAddress: '1 Main St',
  notes: '',
};

async function mount(props: Partial<WcClientForm> = {}): Promise<WcClientForm> {
  const el = document.createElement('wc-client-form');
  Object.assign(el, { value: EMPTY_CLIENT_FORM }, props);
  document.body.appendChild(el);
  await el.updateComplete;
  return el;
}

describe('validateClientForm', () => {
  it('requires a name', () => {
    expect(validateClientForm({ ...filled, name: '   ' }).name).toBe('Name is required');
    expect(validateClientForm(filled).name).toBeUndefined();
  });

  it('accepts an address the CLI would accept, however odd', () => {
    // `nigel client add` does not shape-check an email and neither does the
    // route; a stricter web form would make the two surfaces disagree.
    expect(validateClientForm({ ...filled, email: 'not-an-email' })).toEqual({});
    expect(validateClientForm({ ...filled, email: '' })).toEqual({});
  });
});

describe('wc-client-form', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('collects all four fields', async () => {
    const el = await mount();
    for (const hook of ['[data-name]', '[data-email]', '[data-address]', '[data-notes]']) {
      expect(el.shadowRoot?.querySelector(hook), hook).toBeTruthy();
    }
  });

  it('emits the whole value on every edit', async () => {
    const el = await mount({ value: filled });
    const seen: ClientFormValue[] = [];
    el.addEventListener('nc-client-form-change', (event) =>
      seen.push((event as CustomEvent<NcClientFormChangeDetail>).detail.value),
    );

    const input = el.shadowRoot?.querySelector<HTMLInputElement>('[data-address]');
    input!.value = '2 Elm St';
    input!.dispatchEvent(new Event('input'));

    expect(seen).toEqual([{ ...filled, billingAddress: '2 Elm St' }]);
  });

  it('says what a missing email costs, and stops saying it once one is typed', async () => {
    const empty = await mount();
    expect(empty.shadowRoot?.querySelector('[data-email-hint]')?.textContent).toContain(
      'cannot be sent',
    );

    const filledIn = await mount({ value: filled });
    expect(filledIn.shadowRoot?.querySelector('[data-email-hint]')).toBeNull();
  });

  it('renders a field error beside its field', async () => {
    const el = await mount({ errors: { name: 'Name is required' } });
    expect(el.shadowRoot?.querySelector('.error')?.textContent?.trim()).toBe(
      'Name is required',
    );
  });

  it('disables every control while a save is in flight', async () => {
    const el = await mount({ value: filled, disabled: true });
    const controls = [...(el.shadowRoot?.querySelectorAll('[data-name],[data-email],[data-address],[data-notes]') ?? [])];
    expect(controls).toHaveLength(4);
    expect(controls.every((control) => control.hasAttribute('disabled'))).toBe(true);
  });
});

describePreviewA11y(preview);
