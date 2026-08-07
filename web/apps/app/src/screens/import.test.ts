import { describe, it, expect, afterEach, vi } from 'vitest';
import './import.js';
import type { NigelImportScreen } from './import.js';
import {
  GENERIC_FORMAT_CHOICE,
  type ImportFormValue,
  type NcImportChangeDetail,
  type WcCountGrid,
  type WcDropzone,
  type WcImportForm,
  type WcSampleTable,
} from '@nigel/ui';

import { ApiError } from '../api/index.js';
import { UPLOAD_NOT_FOUND } from '../api/types.js';
import type { Account, ImporterFormat } from '../api/types.js';
import {
  EMPTY_IMPORT_CONFIRMATION,
  EMPTY_IMPORT_PREVIEW,
  FakeApiClient,
} from '../__mocks__/fake-api-client.js';

/**
 * 423 and 401 are deliberately untested here. The shell gates both before a
 * screen element is ever constructed (`boot` never reaches `ready` while
 * locked), so a lock test on this screen would assert behaviour that cannot
 * happen and would quietly pass forever.
 */

const ACCOUNTS: Account[] = [
  {
    id: 1,
    name: 'BofA Checking',
    accountType: 'checking',
    institution: 'BofA',
    lastFour: '1234',
  },
  {
    id: 2,
    name: 'BofA Credit Card',
    accountType: 'credit_card',
    institution: 'BofA',
    lastFour: '9876',
  },
];

const FORMATS: ImporterFormat[] = [
  { key: 'bofa_checking', name: 'Bank of America Checking', accountTypes: ['checking'] },
];

function statement(name = 'april-2025.csv', size = 8214): File {
  const file = new File(['date,description,amount\n'], name);
  Object.defineProperty(file, 'size', { value: size });
  return file;
}

function client(): FakeApiClient {
  const fake = new FakeApiClient();
  fake.accounts = ACCOUNTS;
  fake.importFormats = FORMATS;
  fake.importPreview = {
    ...EMPTY_IMPORT_PREVIEW,
    imported: 42,
    skipped: 3,
    malformed: 1,
    format: 'bofa_checking',
    sample: [
      { date: '2025-04-01', description: 'ACME CORP', amount: 3000 },
      { date: '2025-04-03', description: 'ADOBE', amount: -59.99 },
    ],
  };
  fake.importConfirmation = {
    ...EMPTY_IMPORT_CONFIRMATION,
    imported: 42,
    skipped: 3,
    malformed: 1,
    format: 'bofa_checking',
    importId: 7,
    categorized: 38,
    stillFlagged: 6,
    snapshot: '/tmp/nigel/snapshots/pre-import-20250401-120000.db',
  };
  return fake;
}

async function settle(el: NigelImportScreen): Promise<void> {
  await el.updateComplete;
  await new Promise((resolve) => setTimeout(resolve, 0));
  await el.updateComplete;
}

async function mount(
  fake: FakeApiClient = client(),
): Promise<{ el: NigelImportScreen; fake: FakeApiClient }> {
  const el = document.createElement('nigel-import-screen');
  el.client = fake;
  document.body.appendChild(el);
  await settle(el);
  return { el, fake };
}

function dropzone(el: NigelImportScreen): WcDropzone {
  const found = el.shadowRoot?.querySelector<WcDropzone>('wc-dropzone');
  if (!found) throw new Error('no dropzone on screen');
  return found;
}

function form(el: NigelImportScreen): WcImportForm {
  const found = el.shadowRoot?.querySelector<WcImportForm>('wc-import-form');
  if (!found) throw new Error('no import form on screen');
  return found;
}

/** The screen's buttons, by their visible text. */
function button(el: NigelImportScreen, text: string): HTMLButtonElement | null {
  const buttons = [...(el.shadowRoot?.querySelectorAll('button') ?? [])];
  return (
    (buttons.find((b) => b.textContent?.trim().startsWith(text)) as HTMLButtonElement) ??
    null
  );
}

function panelHeadings(el: NigelImportScreen): string[] {
  return [...(el.shadowRoot?.querySelectorAll('wc-panel') ?? [])].map(
    (panel) => panel.getAttribute('heading') ?? '',
  );
}

async function choose(el: NigelImportScreen, file = statement()): Promise<void> {
  dropzone(el).dispatchEvent(
    new CustomEvent('nc-file-select', {
      detail: { file },
      bubbles: true,
      composed: true,
    }),
  );
  await settle(el);
}

async function setForm(
  el: NigelImportScreen,
  patch: Partial<ImportFormValue>,
): Promise<void> {
  const current: ImportFormValue = form(el).value;
  form(el).dispatchEvent(
    new CustomEvent<NcImportChangeDetail>('nc-import-change', {
      detail: { value: { ...current, ...patch } },
      bubbles: true,
      composed: true,
    }),
  );
  await settle(el);
}

async function click(el: NigelImportScreen, text: string): Promise<void> {
  const target = button(el, text);
  if (!target) throw new Error(`no "${text}" button on screen`);
  target.click();
  await settle(el);
}

/** Choose a file, name an account, and preview — the common opening. */
async function toPreview(
  el: NigelImportScreen,
  patch: Partial<ImportFormValue> = {},
): Promise<void> {
  await choose(el);
  await setForm(el, { account: 'BofA Checking', ...patch });
  await click(el, 'Preview');
}

function bodyOf(call: string): Record<string, unknown> {
  return JSON.parse(call.slice(call.indexOf(':') + 1));
}

describe('nigel-import-screen', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('loads accounts, formats and profiles on enter', async () => {
    const { fake } = await mount();
    expect(fake.calls).toEqual(['getAccounts', 'getImportFormats', 'getCsvProfiles']);
  });

  it('walks select, preview and confirm', async () => {
    const { el, fake } = await mount();

    await toPreview(el);

    expect(fake.calls).toContain('uploadImport:april-2025.csv');
    expect(panelHeadings(el)).toContain('Preview');
    const sample = el.shadowRoot?.querySelector<WcSampleTable>('wc-sample-table');
    expect(sample?.rows).toHaveLength(2);

    await click(el, 'Import 42 transactions');

    expect(panelHeadings(el)).toEqual(['Import complete']);
    const counts = el.shadowRoot?.querySelector<WcCountGrid>('wc-count-grid');
    expect(counts?.items.map((item) => [item.label, item.value])).toEqual([
      ['Imported', 42],
      ['Duplicates', 3],
      ['Malformed', 1],
      ['Categorized', 38],
      ['Still flagged', 6],
    ]);
  });

  it('calls upload, preview and confirm in that order', async () => {
    const { el, fake } = await mount();
    await toPreview(el);
    await click(el, 'Import 42');

    const names = fake.calls
      .map((call) => call.split(':')[0])
      .filter((name) =>
        ['uploadImport', 'previewImport', 'confirmImport'].includes(name),
      );
    expect(names).toEqual(['uploadImport', 'previewImport', 'confirmImport']);
  });

  it('shows the snapshot path and a link to the flagged review', async () => {
    const { el } = await mount();
    await toPreview(el);
    await click(el, 'Import 42');

    expect(el.shadowRoot?.querySelector('.snapshot')?.textContent).toContain(
      'pre-import-20250401-120000.db',
    );
    const link = el.shadowRoot?.querySelector('a[href="#/review"]');
    expect(link?.textContent).toContain('Review 6 flagged');
  });

  it('offers no review link when nothing is flagged', async () => {
    const fake = client();
    fake.importConfirmation = { ...fake.importConfirmation, stillFlagged: 0 };
    const { el } = await mount(fake);
    await toPreview(el);
    await click(el, 'Import 42');

    expect(el.shadowRoot?.querySelector('a[href="#/review"]')).toBeNull();
  });

  it('blocks the confirm for a duplicate file', async () => {
    const fake = client();
    fake.importPreview = { ...EMPTY_IMPORT_PREVIEW, duplicateFile: true };
    const { el } = await mount(fake);

    await toPreview(el);

    expect(panelHeadings(el)).toContain('Already imported');
    expect(el.shadowRoot?.querySelector('wc-notice-bar')?.getAttribute('variant')).toBe(
      'warning',
    );
    expect(button(el, 'Import')).toBeNull();
    expect(fake.calls.some((call) => call.startsWith('confirmImport'))).toBe(false);
  });

  it('sends the mapping and the profile name for a generic CSV', async () => {
    const { el, fake } = await mount();

    await toPreview(el, {
      format: GENERIC_FORMAT_CHOICE,
      mapping: { dateCol: 3, descCol: 1, amountCol: 4, dateFormat: '%d/%m/%Y' },
      saveProfile: 'chase',
    });
    await click(el, 'Import 42');

    const confirm = fake.calls.find((call) => call.startsWith('confirmImport'));
    const body = bodyOf(confirm!);
    expect(body.mapping).toEqual({
      dateCol: 3,
      descCol: 1,
      amountCol: 4,
      dateFormat: '%d/%m/%Y',
    });
    expect(body.saveProfile).toBe('chase');
    expect(body).not.toHaveProperty('format');
  });

  it('re-reads the profile list after saving one', async () => {
    const { el, fake } = await mount();
    await toPreview(el, {
      format: GENERIC_FORMAT_CHOICE,
      saveProfile: 'chase',
    });
    await click(el, 'Import 42');

    expect(fake.csvProfiles.map((p) => p.name)).toContain('chase');
    expect(fake.calls.filter((call) => call === 'getCsvProfiles')).toHaveLength(2);
  });

  it('sends neither format nor mapping when detecting', async () => {
    const { el, fake } = await mount();
    await toPreview(el);

    const body = bodyOf(fake.calls.find((c) => c.startsWith('previewImport'))!);
    expect(body).not.toHaveProperty('format');
    expect(body).not.toHaveProperty('mapping');
    expect(body.account).toBe('BofA Checking');
  });

  it('sends an explicit format alone', async () => {
    const { el, fake } = await mount();
    await toPreview(el, { format: 'bofa_checking' });

    const body = bodyOf(fake.calls.find((c) => c.startsWith('previewImport'))!);
    expect(body.format).toBe('bofa_checking');
    expect(body).not.toHaveProperty('mapping');
  });

  it('surfaces a bad mapping under the mapping form and allows another try', async () => {
    const fake = client();
    fake.previewErrorOnce = new ApiError({
      code: 'bad_request',
      rawCode: 'bad_request',
      message: 'Column 9 is past the end of every row.',
      status: 400,
    });
    const { el } = await mount(fake);

    await toPreview(el, { format: GENERIC_FORMAT_CHOICE });

    expect(form(el).mappingError).toContain('Column 9');
    expect(panelHeadings(el)).not.toContain('Preview');
    // The file is still chosen, so correcting the columns is the only work left.
    expect(dropzone(el).filename).toBe('april-2025.csv');

    await click(el, 'Preview');
    expect(panelHeadings(el)).toContain('Preview');
    // The upload was cached: the retry cost one request, not two.
    expect(fake.calls.filter((c) => c.startsWith('uploadImport'))).toHaveLength(1);
  });

  it('reuses the upload when only the mapping changed', async () => {
    const { el, fake } = await mount();
    await toPreview(el, { format: GENERIC_FORMAT_CHOICE });
    await setForm(el, { mapping: { dateCol: 2, descCol: 0, amountCol: 3, dateFormat: '%Y-%m-%d' } });
    await click(el, 'Preview');

    expect(fake.calls.filter((c) => c.startsWith('uploadImport'))).toHaveLength(1);
    expect(fake.calls.filter((c) => c.startsWith('previewImport'))).toHaveLength(2);
  });

  it('re-uploads once when the upload has expired', async () => {
    const fake = client();
    fake.previewErrorOnce = new ApiError({
      code: 'not_found',
      rawCode: 'not_found',
      message: 'gone',
      status: 404,
      details: { reason: UPLOAD_NOT_FOUND },
    });
    const { el } = await mount(fake);

    await toPreview(el);

    // Recovered without saying anything: the file never left the browser.
    expect(panelHeadings(el)).toContain('Preview');
    expect(fake.calls.filter((c) => c.startsWith('uploadImport'))).toHaveLength(2);
    expect(dropzone(el).error).toBe('');
  });

  it('gives up after one re-upload and says the upload expired', async () => {
    const fake = client();
    fake.previewError = new ApiError({
      code: 'not_found',
      rawCode: 'not_found',
      message: 'gone',
      status: 404,
      details: { reason: UPLOAD_NOT_FOUND },
    });
    const { el } = await mount(fake);

    await toPreview(el);

    expect(dropzone(el).error).toContain('expired');
    expect(fake.calls.filter((c) => c.startsWith('uploadImport'))).toHaveLength(2);
  });

  it('puts an oversize rejection from the server under the dropzone', async () => {
    const fake = client();
    fake.uploadError = new ApiError({
      code: 'payload_too_large',
      rawCode: 'payload_too_large',
      message: 'That file is over the 25 MB limit.',
      status: 413,
    });
    const { el } = await mount(fake);

    await toPreview(el);

    expect(dropzone(el).error).toContain('25 MB');
  });

  it('never uploads a file the dropzone already refused', async () => {
    const { el, fake } = await mount();
    await setForm(el, { account: 'BofA Checking' });

    dropzone(el).dispatchEvent(
      new CustomEvent('nc-file-error', {
        detail: { message: 'nigel reads .csv, .xlsx, .xls statements.' },
        bubbles: true,
        composed: true,
      }),
    );
    await settle(el);

    expect(dropzone(el).error).toContain('.csv');
    expect(fake.calls.some((call) => call.startsWith('uploadImport'))).toBe(false);
    expect(button(el, 'Preview')?.disabled).toBe(true);
  });

  it('puts a missing cargo feature under the format select', async () => {
    const fake = client();
    fake.previewError = new ApiError({
      code: 'feature_disabled',
      rawCode: 'feature_disabled',
      message: 'This build has no Gusto payroll support.',
      status: 501,
    });
    const { el } = await mount(fake);

    await toPreview(el, { format: 'gusto_payroll' });

    expect(form(el).formatError).toContain('Gusto');
  });

  it('puts an unknown account under the account select and toasts it', async () => {
    const fake = client();
    fake.previewError = new ApiError({
      code: 'not_found',
      rawCode: 'not_found',
      message: 'No account named that.',
      status: 404,
    });
    const { el } = await mount(fake);
    const toasted = vi.fn();
    el.addEventListener('nc-toast', toasted);

    await toPreview(el);

    expect(form(el).accountError).toContain('No account');
    expect(toasted).toHaveBeenCalled();
  });

  it('resets for a second import without a reload', async () => {
    const { el, fake } = await mount();
    await toPreview(el);
    await click(el, 'Import 42');

    await click(el, 'Import another');

    expect(panelHeadings(el)).toEqual(['Import a statement']);
    expect(dropzone(el).filename).toBe('');
    // The account survives: a second statement for the same account is the
    // ordinary next thing to do.
    expect(form(el).value.account).toBe('BofA Checking');

    await choose(el, statement('may-2025.csv'));
    await click(el, 'Preview');
    await click(el, 'Import 42');

    expect(panelHeadings(el)).toEqual(['Import complete']);
    const uploads = fake.calls.filter((c) => c.startsWith('uploadImport'));
    expect(uploads).toEqual(['uploadImport:april-2025.csv', 'uploadImport:may-2025.csv']);

    // The second confirm used a fresh upload, not the one the first consumed.
    const confirms = fake.calls.filter((c) => c.startsWith('confirmImport'));
    expect(bodyOf(confirms[0]).uploadId).not.toBe(bodyOf(confirms[1]).uploadId);
  });

  it('drops a stale preview when the file changes', async () => {
    const { el } = await mount();
    await toPreview(el);
    expect(panelHeadings(el)).toContain('Preview');

    await choose(el, statement('may-2025.csv'));

    expect(panelHeadings(el)).not.toContain('Preview');
  });

  it('drops a stale preview when the format changes', async () => {
    const { el } = await mount();
    await toPreview(el);

    await setForm(el, { format: 'bofa_checking' });

    // The old preview described a different reading of the same bytes.
    expect(panelHeadings(el)).not.toContain('Preview');
  });

  it('preselects the only account there is', async () => {
    const fake = client();
    fake.accounts = [ACCOUNTS[0]];
    const { el } = await mount(fake);

    expect(form(el).value.account).toBe('BofA Checking');
  });

  it('leaves the account unchosen when there is more than one', async () => {
    const { el } = await mount();
    expect(form(el).value.account).toBe('');
    expect(button(el, 'Preview')?.disabled).toBe(true);
  });

  it('still renders the form when the profile list fails', async () => {
    const fake = client();
    fake.csvProfilesError = new ApiError({
      code: 'internal',
      rawCode: 'internal',
      message: 'profiles are down',
      status: 500,
    });
    const { el } = await mount(fake);

    // One failed list is no reason to withhold the other two.
    expect(form(el).accounts).toHaveLength(2);
    expect(form(el).formats).toHaveLength(1);
    expect(el.shadowRoot?.querySelector('.load-error')?.textContent).toContain(
      'profiles are down',
    );
  });
});
