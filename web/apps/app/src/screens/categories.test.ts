import { describe, it, expect, afterEach, vi } from 'vitest';
import './categories.js';
import type { NigelCategoriesScreen } from './categories.js';
import type {
  WcCategoryForm,
  WcManagerDialog,
  WcManagerLayout,
  WcManagerTable,
} from '@nigel/ui';
import { conflictError, FakeApiClient } from '../__mocks__/fake-api-client.js';
import { initializeAppStore, resetAppStore } from '../state/app-store.js';
import type { CategoryRow } from '../api/types.js';
import type { ScreenId } from './registry.js';

const CATEGORIES: CategoryRow[] = [
  {
    id: 3,
    name: 'Consulting income',
    categoryType: 'income',
    taxLine: 'Gross receipts',
    formLine: '1120S-1a',
  },
  {
    id: 12,
    name: 'Software / Subscriptions',
    categoryType: 'expense',
    taxLine: 'Other expenses',
    formLine: '1120S-19',
  },
  {
    id: 14,
    name: 'Meals / Entertainment',
    categoryType: 'expense',
    taxLine: null,
    formLine: null,
  },
];

function client(categories: CategoryRow[] = CATEGORIES): FakeApiClient {
  const fake = new FakeApiClient();
  fake.categories = categories.map((category) => ({ ...category }));
  return fake;
}

async function settle(el: NigelCategoriesScreen): Promise<void> {
  await el.updateComplete;
  await new Promise((resolve) => setTimeout(resolve, 0));
  await el.updateComplete;
}

interface Mounted {
  el: NigelCategoriesScreen;
  fake: FakeApiClient;
  navigations: { screen: ScreenId; params?: URLSearchParams }[];
}

async function mount(fake: FakeApiClient = client()): Promise<Mounted> {
  // The screen reads the books profile off the app store for its description.
  resetAppStore();
  const store = initializeAppStore(fake, { reload: () => {} });
  await store.refreshStatus();
  fake.calls.length = 0;

  const navigations: Mounted['navigations'] = [];
  const el = document.createElement('nigel-categories-screen');
  el.client = fake;
  el.navigate = (screen, params) => navigations.push({ screen, params });
  document.body.appendChild(el);
  await settle(el);
  return { el, fake, navigations };
}

function layout(el: NigelCategoriesScreen): WcManagerLayout {
  const found = el.shadowRoot?.querySelector<WcManagerLayout>('wc-manager-layout');
  if (!found) throw new Error('no layout on screen');
  return found;
}

function table(el: NigelCategoriesScreen): WcManagerTable {
  const found = el.shadowRoot?.querySelector<WcManagerTable>('wc-manager-table');
  if (!found) throw new Error('no table on screen');
  return found;
}

function dialog(el: NigelCategoriesScreen): WcManagerDialog | null {
  return el.shadowRoot?.querySelector<WcManagerDialog>('wc-manager-dialog') ?? null;
}

function form(el: NigelCategoriesScreen): WcCategoryForm {
  const found = dialog(el)?.querySelector<WcCategoryForm>('wc-category-form');
  if (!found) throw new Error('no category form on screen');
  return found;
}

async function type(
  el: NigelCategoriesScreen,
  hook: string,
  value: string,
): Promise<void> {
  const field = form(el).shadowRoot?.querySelector<HTMLInputElement>(hook);
  if (!field) throw new Error(`no ${hook} in the form`);
  field.value = value;
  field.dispatchEvent(new Event('input'));
  await settle(el);
}

async function openAdd(el: NigelCategoriesScreen): Promise<void> {
  layout(el).dispatchEvent(new CustomEvent('nc-manager-add'));
  await settle(el);
}

async function rowAction(
  el: NigelCategoriesScreen,
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

async function save(el: NigelCategoriesScreen): Promise<void> {
  dialog(el)?.dispatchEvent(new CustomEvent('nc-manager-save'));
  await settle(el);
}

async function confirmDeletion(answer: boolean): Promise<void> {
  const ui = await import('@nigel/ui');
  vi.spyOn(ui, 'confirmDialog').mockResolvedValue(answer);
}

describe('nigel-categories-screen', () => {
  afterEach(() => {
    document.body.innerHTML = '';
    vi.restoreAllMocks();
  });

  it('describes the screen for the books profile', async () => {
    const { el } = await mount();
    expect(layout(el).getAttribute('description')).toContain('1120-S');

    const personal = client();
    personal.status = { ...personal.status, profile: 'personal' };
    const { el: personalEl } = await mount(personal);
    expect(layout(personalEl).getAttribute('description')).not.toContain('1120-S');
  });

  it('lists the chart of accounts in the order the server sends it', async () => {
    const { el } = await mount();
    expect(table(el).rows.map((row) => row.cells[0])).toEqual([
      'Consulting income',
      'Software / Subscriptions',
      'Meals / Entertainment',
    ]);
  });

  it('renders every field, with missing ones left null for the table', async () => {
    const { el } = await mount();
    expect(table(el).rows[2].cells).toEqual([
      'Meals / Entertainment',
      'Expense',
      null,
      null,
    ]);
  });

  it('creates a category with all four fields', async () => {
    const { el, fake } = await mount();
    await openAdd(el);
    await type(el, '[data-name]', 'Contract labor');
    await type(el, '[data-tax-line]', 'Contract labor');
    await type(el, '[data-form-line]', '1120S-11');
    await save(el);

    expect(fake.calls).toEqual([
      'getCategories',
      'createCategory:{"name":"Contract labor","categoryType":"expense","taxLine":"Contract labor","formLine":"1120S-11"}',
      'getCategories',
    ]);
  });

  it('sends only the fields an edit changed', async () => {
    const { el, fake } = await mount();
    await rowAction(el, 'edit', 12);
    await type(el, '[data-name]', 'Software');
    await save(el);

    expect(fake.calls[1]).toBe('updateCategory:12:{"name":"Software"}');
  });

  it('clears a tax line with an explicit null', async () => {
    const { el, fake } = await mount();
    await rowAction(el, 'edit', 12);
    await type(el, '[data-tax-line]', '');
    await save(el);

    expect(fake.calls[1]).toBe('updateCategory:12:{"taxLine":null}');
    expect(fake.categories.find((c) => c.id === 12)?.taxLine).toBeNull();
  });

  it('issues no request when an edit changes nothing', async () => {
    // A patch with no recognized field is a 400 by design.
    const { el, fake } = await mount();
    await rowAction(el, 'edit', 12);
    await save(el);

    expect(fake.calls).toEqual(['getCategories']);
    expect(dialog(el)).toBeNull();
  });

  it('suggests the form lines already in use, plus the anchors', async () => {
    const { el } = await mount();
    await openAdd(el);
    expect(form(el).suggestions).toEqual([
      '1120S-19',
      '1120S-1a',
      '1120S-2',
      '1120S-5',
      'excluded',
    ]);
  });

  it('warns about an unrecognized form line without blocking the save', async () => {
    const { el, fake } = await mount();
    await openAdd(el);
    await type(el, '[data-name]', 'Bank fees');
    await type(el, '[data-form-line]', '1120s-19');
    expect(form(el).shadowRoot?.querySelector('.warning')).not.toBeNull();

    await save(el);
    expect(fake.calls[1]).toContain('"formLine":"1120s-19"');
  });

  it('explains a category still used by transactions', async () => {
    await confirmDeletion(true);
    const fake = client();
    fake.deleteCategoryError = conflictError('has_transactions', { count: 37 });
    const { el } = await mount(fake);
    await rowAction(el, 'delete', 12);

    expect(layout(el).error).toBe(
      '37 transactions use this category. Recategorize them first.',
    );
    expect(layout(el).errorActionLabel).toBe('');
  });

  it('points a rules block at the rules that are blocking it', async () => {
    // AC #4: the reason code is what makes this actionable rather than a
    // number the user then has to go hunting for.
    await confirmDeletion(true);
    const fake = client();
    fake.deleteCategoryError = conflictError('has_active_rules', { count: 3 });
    const { el, navigations } = await mount(fake);
    await rowAction(el, 'delete', 12);

    expect(layout(el).error).toBe(
      '3 active rules assign this category. Delete those rules first.',
    );
    expect(layout(el).errorActionLabel).toBe('Show those rules');

    layout(el).dispatchEvent(new CustomEvent('nc-manager-error-action'));
    await settle(el);

    expect(navigations).toHaveLength(1);
    expect(navigations[0].screen).toBe('rules');
    expect(navigations[0].params?.get('categoryId')).toBe('12');
  });

  it('deletes once confirmed, and refetches', async () => {
    await confirmDeletion(true);
    const { el, fake } = await mount();
    await rowAction(el, 'delete', 14);

    expect(fake.calls).toEqual(['getCategories', 'deleteCategory:14', 'getCategories']);
    expect(table(el).rows).toHaveLength(2);
  });

  it('explains a duplicate name in the dialog', async () => {
    const fake = client();
    fake.createCategoryError = conflictError('duplicate_name', { name: 'Software' });
    const { el } = await mount(fake);
    await openAdd(el);
    await type(el, '[data-name]', 'Software');
    await save(el);

    expect(dialog(el)?.error).toBe('A category named “Software” already exists.');
  });

  it('requires a name', async () => {
    const { el, fake } = await mount();
    await openAdd(el);
    await save(el);

    expect(fake.calls).toEqual(['getCategories']);
    expect(form(el).errors.name).toBe('Name is required');
  });
});
