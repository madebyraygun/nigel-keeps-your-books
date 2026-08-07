import { describe, it, expect, afterEach, vi } from 'vitest';
import './accounts.js';
import type { NigelAccountsScreen } from './accounts.js';
import type {
  WcAccountForm,
  WcManagerDialog,
  WcManagerLayout,
  WcManagerTable,
} from '@nigel/ui';
import { ApiError } from '../api/index.js';
import { conflictError, FakeApiClient } from '../__mocks__/fake-api-client.js';
import type { Account } from '../api/types.js';

const ACCOUNTS: Account[] = [
  {
    id: 1,
    name: 'BofA Checking',
    accountType: 'checking',
    institution: 'Bank of America',
    lastFour: '4821',
  },
  {
    id: 2,
    name: 'Gusto Payroll',
    accountType: 'payroll',
    institution: null,
    lastFour: null,
  },
];

function client(accounts: Account[] = ACCOUNTS): FakeApiClient {
  const fake = new FakeApiClient();
  fake.accounts = accounts.map((account) => ({ ...account }));
  return fake;
}

async function settle(el: NigelAccountsScreen): Promise<void> {
  await el.updateComplete;
  await new Promise((resolve) => setTimeout(resolve, 0));
  await el.updateComplete;
}

async function mount(
  fake: FakeApiClient = client(),
): Promise<{ el: NigelAccountsScreen; fake: FakeApiClient }> {
  const el = document.createElement('nigel-accounts-screen');
  el.client = fake;
  document.body.appendChild(el);
  await settle(el);
  return { el, fake };
}

function layout(el: NigelAccountsScreen): WcManagerLayout {
  const found = el.shadowRoot?.querySelector<WcManagerLayout>('wc-manager-layout');
  if (!found) throw new Error('no layout on screen');
  return found;
}

function table(el: NigelAccountsScreen): WcManagerTable {
  const found = el.shadowRoot?.querySelector<WcManagerTable>('wc-manager-table');
  if (!found) throw new Error('no table on screen');
  return found;
}

function dialog(el: NigelAccountsScreen): WcManagerDialog | null {
  return el.shadowRoot?.querySelector<WcManagerDialog>('wc-manager-dialog') ?? null;
}

function form(el: NigelAccountsScreen): WcAccountForm {
  const found = dialog(el)?.querySelector<WcAccountForm>('wc-account-form');
  if (!found) throw new Error('no account form on screen');
  return found;
}

/** Type into a field of the open form the way the component would. */
async function type(
  el: NigelAccountsScreen,
  hook: string,
  value: string,
): Promise<void> {
  const field = form(el).shadowRoot?.querySelector<HTMLInputElement>(hook);
  if (!field) throw new Error(`no ${hook} in the form`);
  field.value = value;
  field.dispatchEvent(new Event('input'));
  await settle(el);
}

async function openAdd(el: NigelAccountsScreen): Promise<void> {
  layout(el).dispatchEvent(new CustomEvent('nc-manager-add'));
  await settle(el);
}

async function rowAction(
  el: NigelAccountsScreen,
  action: string,
  id: number,
): Promise<void> {
  table(el).dispatchEvent(
    new CustomEvent('nc-manager-action', {
      detail: { action, id },
      bubbles: true,
      composed: true,
    }),
  );
  await settle(el);
}

async function save(el: NigelAccountsScreen): Promise<void> {
  dialog(el)?.dispatchEvent(new CustomEvent('nc-manager-save'));
  await settle(el);
}

async function confirmDeletion(answer: boolean): Promise<void> {
  const ui = await import('@nigel/ui');
  vi.spyOn(ui, 'confirmDialog').mockResolvedValue(answer);
}

describe('nigel-accounts-screen', () => {
  afterEach(() => {
    document.body.innerHTML = '';
    vi.restoreAllMocks();
  });

  it('lists the accounts with humanized types', async () => {
    const { el } = await mount();
    expect(table(el).rows.map((row) => row.cells)).toEqual([
      ['BofA Checking', 'Checking', 'Bank of America', '4821'],
      ['Gusto Payroll', 'Payroll', null, null],
    ]);
    expect(layout(el).count).toBe(2);
  });

  it('shows the empty state when there are none', async () => {
    const { el } = await mount(client([]));
    expect(layout(el).empty).toBe(true);
  });

  it('creates an account and then refetches the list', async () => {
    const { el, fake } = await mount();
    await openAdd(el);
    await type(el, '[data-name]', 'Chase Business');
    await type(el, '[data-institution]', 'Chase');
    await type(el, '[data-last-four]', '9921');
    await save(el);

    expect(fake.calls).toEqual([
      'getAccounts',
      'createAccount:{"name":"Chase Business","accountType":"checking","institution":"Chase","lastFour":"9921"}',
      'getAccounts',
    ]);
    expect(dialog(el)).toBeNull();
    expect(table(el).rows).toHaveLength(3);
  });

  it('sends empty optional fields as null rather than as empty strings', async () => {
    const { el, fake } = await mount();
    await openAdd(el);
    await type(el, '[data-name]', 'Cash');
    await save(el);

    expect(fake.calls[1]).toContain('"institution":null');
    expect(fake.calls[1]).toContain('"lastFour":null');
  });

  it('renames through the patch route, sending only the name', async () => {
    const { el, fake } = await mount();
    await rowAction(el, 'rename', 1);
    await type(el, '[data-name]', 'BofA Business Checking');
    await save(el);

    expect(fake.calls).toEqual([
      'getAccounts',
      'renameAccount:1:{"name":"BofA Business Checking"}',
      'getAccounts',
    ]);
  });

  it('issues no request when a rename changes nothing', async () => {
    // The only thing such a request could do is fail on the account's own name.
    const { el, fake } = await mount();
    await rowAction(el, 'rename', 1);
    await save(el);

    expect(fake.calls).toEqual(['getAccounts']);
    expect(dialog(el)).toBeNull();
  });

  it('blocks a malformed last four before it reaches the server', async () => {
    const { el, fake } = await mount();
    await openAdd(el);
    await type(el, '[data-name]', 'Chase');
    await type(el, '[data-last-four]', '12a');
    await save(el);

    expect(fake.calls).toEqual(['getAccounts']);
    expect(form(el).errors.lastFour).toBe('Last four must be exactly 4 digits');
    expect(dialog(el)).not.toBeNull();
  });

  it('requires a name before it will send anything', async () => {
    const { el, fake } = await mount();
    await openAdd(el);
    await save(el);

    expect(fake.calls).toEqual(['getAccounts']);
    expect(form(el).errors.name).toBe('Name is required');
  });

  it('deletes once confirmed, and refetches', async () => {
    await confirmDeletion(true);
    const { el, fake } = await mount();
    await rowAction(el, 'delete', 2);

    expect(fake.calls).toEqual(['getAccounts', 'deleteAccount:2', 'getAccounts']);
    expect(table(el).rows).toHaveLength(1);
  });

  it('deletes nothing when the confirmation is refused', async () => {
    await confirmDeletion(false);
    const { el, fake } = await mount();
    await rowAction(el, 'delete', 2);

    expect(fake.calls).toEqual(['getAccounts']);
  });

  it('explains a blocked delete inline, with the count, and keeps the row', async () => {
    // AC #1 and AC #4: the reason code becomes our sentence, in the screen's
    // alert region, because the confirm dialog is already gone by then.
    await confirmDeletion(true);
    const fake = client();
    fake.deleteAccountError = conflictError('has_transactions', {
      count: 5,
      message: 'Cannot delete: account has 5 transactions',
    });
    const { el } = await mount(fake);
    await rowAction(el, 'delete', 1);

    expect(layout(el).error).toBe(
      'This account has 5 transactions. Nigel will not delete an account that still has activity.',
    );
    expect(table(el).rows).toHaveLength(2);
  });

  it('explains a duplicate name in the dialog, and leaves it open', async () => {
    const fake = client();
    fake.createAccountError = conflictError('duplicate_name', {
      name: 'BofA Checking',
    });
    const { el } = await mount(fake);
    await openAdd(el);
    await type(el, '[data-name]', 'BofA Checking');
    await save(el);

    expect(dialog(el)?.error).toBe('An account named “BofA Checking” already exists.');
    expect(dialog(el)).not.toBeNull();
  });

  it('shows a 400 as the server worded it', async () => {
    // It names the offending value and the legal set; ours would be worse.
    const fake = client();
    fake.createAccountError = new ApiError({
      code: 'bad_request',
      rawCode: 'bad_request',
      message: 'Invalid account type: brokerage (must be one of: checking, credit_card)',
      status: 400,
    });
    const { el } = await mount(fake);
    await openAdd(el);
    await type(el, '[data-name]', 'Brokerage');
    await save(el);

    expect(dialog(el)?.error).toBe(
      'Invalid account type: brokerage (must be one of: checking, credit_card)',
    );
  });

  it('offers a retry when the list will not load', async () => {
    const fake = client();
    fake.accountsError = new ApiError({
      code: 'internal',
      rawCode: 'internal',
      message: 'database is locked',
      status: 500,
    });
    const { el } = await mount(fake);

    expect(layout(el).error).toBe('database is locked');
    expect(layout(el).errorActionLabel).toBe('Try again');

    fake.accountsError = null;
    layout(el).dispatchEvent(new CustomEvent('nc-manager-error-action'));
    await settle(el);

    expect(layout(el).error).toBeNull();
    expect(table(el).rows).toHaveLength(2);
  });

  it('dismisses a guardrail on request', async () => {
    await confirmDeletion(true);
    const fake = client();
    fake.deleteAccountError = conflictError('has_transactions', { count: 5 });
    const { el } = await mount(fake);
    await rowAction(el, 'delete', 1);

    layout(el).dispatchEvent(new CustomEvent('nc-manager-error-dismiss'));
    await settle(el);
    expect(layout(el).error).toBeNull();
  });

  it('shows only the name field when renaming', async () => {
    const { el } = await mount();
    await rowAction(el, 'rename', 1);
    expect(form(el).mode).toBe('rename');
    expect(form(el).shadowRoot?.querySelector('[data-type]')).toBeNull();
  });
});
