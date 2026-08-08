import { describe, it, expect, afterEach, vi } from 'vitest';
import './clients.js';
import type { NigelClientsScreen } from './clients.js';
import type {
  WcClientForm,
  WcManagerDialog,
  WcManagerLayout,
  WcManagerTable,
} from '@nigel/ui';
import { conflictError, FakeApiClient } from '../__mocks__/fake-api-client.js';
import type { Client } from '../api/types.js';
import type { ScreenId } from './registry.js';

const CLIENTS: Client[] = [
  {
    id: 1,
    name: 'Acme Co',
    email: 'ap@acme.test',
    billingAddress: '1 Main St',
    notes: null,
  },
  { id: 2, name: 'Globex', email: null, billingAddress: null, notes: null },
];

function client(clients: Client[] = CLIENTS): FakeApiClient {
  const fake = new FakeApiClient();
  fake.clients = clients.map((row) => ({ ...row }));
  return fake;
}

async function settle(el: NigelClientsScreen): Promise<void> {
  await el.updateComplete;
  await new Promise((resolve) => setTimeout(resolve, 0));
  await el.updateComplete;
}

interface Mounted {
  el: NigelClientsScreen;
  fake: FakeApiClient;
  routes: { screen: ScreenId; params: string }[];
}

async function mount(fake: FakeApiClient = client()): Promise<Mounted> {
  const routes: { screen: ScreenId; params: string }[] = [];
  const el = document.createElement('nigel-clients-screen');
  el.client = fake;
  el.navigate = (screen, params) =>
    routes.push({ screen, params: params?.toString() ?? '' });
  document.body.appendChild(el);
  await settle(el);
  return { el, fake, routes };
}

function layout(el: NigelClientsScreen): WcManagerLayout {
  const found = el.shadowRoot?.querySelector<WcManagerLayout>('wc-manager-layout');
  if (!found) throw new Error('no layout on screen');
  return found;
}

function table(el: NigelClientsScreen): WcManagerTable {
  const found = el.shadowRoot?.querySelector<WcManagerTable>('wc-manager-table');
  if (!found) throw new Error('no table on screen');
  return found;
}

function dialog(el: NigelClientsScreen): WcManagerDialog | null {
  return el.shadowRoot?.querySelector<WcManagerDialog>('wc-manager-dialog') ?? null;
}

function form(el: NigelClientsScreen): WcClientForm {
  const found = dialog(el)?.querySelector<WcClientForm>('wc-client-form');
  if (!found) throw new Error('no client form on screen');
  return found;
}

async function type(
  el: NigelClientsScreen,
  hook: string,
  value: string,
): Promise<void> {
  const field = form(el).shadowRoot?.querySelector<HTMLInputElement>(hook);
  if (!field) throw new Error(`no ${hook} in the form`);
  field.value = value;
  field.dispatchEvent(new Event('input'));
  await settle(el);
}

async function openAdd(el: NigelClientsScreen): Promise<void> {
  layout(el).dispatchEvent(new CustomEvent('nc-manager-add'));
  await settle(el);
}

async function rowAction(
  el: NigelClientsScreen,
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

async function save(el: NigelClientsScreen): Promise<void> {
  dialog(el)?.dispatchEvent(new CustomEvent('nc-manager-save'));
  await settle(el);
}

async function answerConfirm(answer: boolean): Promise<void> {
  const ui = await import('@nigel/ui');
  vi.spyOn(ui, 'confirmDialog').mockResolvedValue(answer);
}

describe('nigel-clients-screen', () => {
  afterEach(() => {
    document.body.innerHTML = '';
    vi.restoreAllMocks();
  });

  it('lists the clients, with an em dash for the ones missing a field', async () => {
    const { el } = await mount();
    expect(table(el).rows.map((row) => row.cells)).toEqual([
      ['Acme Co', 'ap@acme.test', '1 Main St'],
      ['Globex', null, null],
    ]);
    expect(layout(el).count).toBe(2);
  });

  it('shows the empty state when there are none', async () => {
    const { el } = await mount(client([]));
    expect(layout(el).empty).toBe(true);
  });

  it('creates a client and then refetches the list', async () => {
    const { el, fake } = await mount();
    await openAdd(el);
    await type(el, '[data-name]', 'Initech');
    await save(el);

    expect(fake.calls).toContain('createClient:Initech');
    // The refetch is the point: no optimistic splice, because the list is
    // sorted by name and the server is the authority on where a new row lands.
    expect(fake.calls.filter((call) => call === 'getClients')).toHaveLength(2);
    expect(dialog(el)).toBeNull();
  });

  it('sends an empty optional field as null rather than an empty string', async () => {
    const { el, fake } = await mount();
    await openAdd(el);
    await type(el, '[data-name]', 'Initech');
    await save(el);

    const created = fake.clients.find((row) => row.name === 'Initech');
    expect(created?.email).toBeNull();
    expect(created?.billingAddress).toBeNull();
  });

  it('prefills the edit form and sends only what changed', async () => {
    const { el, fake } = await mount();
    await rowAction(el, 'edit', 1);
    expect(form(el).value.name).toBe('Acme Co');
    expect(form(el).value.email).toBe('ap@acme.test');

    await type(el, '[data-address]', '2 Elm St');
    await save(el);

    expect(fake.calls).toContain('updateClient:1:{"billingAddress":"2 Elm St"}');
  });

  it('closes rather than sending an all-absent patch', async () => {
    // An empty PATCH is a 400: a save with nothing changed is a close.
    const { el, fake } = await mount();
    await rowAction(el, 'edit', 1);
    await save(el);

    expect(fake.calls.some((call) => call.startsWith('updateClient'))).toBe(false);
    expect(dialog(el)).toBeNull();
  });

  it('renders a duplicate name in the dialog, beside the field', async () => {
    const fake = client();
    fake.takenClientNames.add('Globex');
    const { el } = await mount(fake);

    await openAdd(el);
    await type(el, '[data-name]', 'Globex');
    await save(el);

    expect(dialog(el)?.error).toBe('A client named “Globex” already exists.');
    expect(layout(el).error).toBeNull();
  });

  it('deletes behind a confirmation, and does nothing when it is declined', async () => {
    await answerConfirm(false);
    const { el, fake } = await mount();
    await rowAction(el, 'delete', 2);
    expect(fake.calls.some((call) => call.startsWith('deleteClient'))).toBe(false);

    await answerConfirm(true);
    await rowAction(el, 'delete', 2);
    expect(fake.calls).toContain('deleteClient:2');
    expect(fake.calls.filter((call) => call === 'getClients')).toHaveLength(2);
  });

  it('renders a blocked delete in the layout with the count and a way through', async () => {
    // `confirmDialog()` has resolved and removed itself by the time the request
    // fails, so there is no dialog left for the refusal to appear in.
    await answerConfirm(true);
    const fake = client();
    fake.clientInvoiceCounts[1] = 7;
    const { el, routes } = await mount(fake);

    await rowAction(el, 'delete', 1);

    expect(layout(el).error).toContain('7 invoices bill this client');
    expect(layout(el).errorActionLabel).toBe('Show those invoices');

    layout(el).dispatchEvent(new CustomEvent('nc-manager-error-action'));
    expect(routes).toEqual([{ screen: 'invoices', params: 'clientId=1' }]);
  });

  it('offers the invoices of any client from its row', async () => {
    const { el, routes } = await mount();
    await rowAction(el, 'invoices', 2);
    expect(routes).toEqual([{ screen: 'invoices', params: 'clientId=2' }]);
  });

  it('offers a retry when the list itself would not load', async () => {
    const fake = client();
    fake.clientsError = conflictError('nope', { message: 'Could not read the clients' });
    const { el } = await mount(fake);

    expect(layout(el).error).toBe('Could not read the clients');
    expect(layout(el).errorActionLabel).toBe('Try again');
  });

  it('refuses to save a client with no name', async () => {
    const { el, fake } = await mount();
    await openAdd(el);
    await save(el);

    expect(form(el).errors.name).toBe('Name is required');
    expect(fake.calls.some((call) => call.startsWith('createClient'))).toBe(false);
  });
});
