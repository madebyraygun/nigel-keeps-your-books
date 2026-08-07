import { describe, it, expect } from 'vitest';
import {
  DEFAULT_CSV_MAPPING,
  EMPTY_IMPORT_FORM,
  GENERIC_FORMAT_CHOICE,
  type GenericCsvMapping,
  type ImportFormValue,
} from '@nigel/ui';

import { ApiError } from '../api/index.js';
import { GENERIC_FORMAT, UPLOAD_NOT_FOUND } from '../api/types.js';
import type {
  CsvProfile,
  GenericCsvConfig,
  ImportConfirmation,
  ImporterFormat,
  ImportPreview,
} from '../api/types.js';
import {
  confirmRequestBody,
  formatLabel,
  importRequestBody,
  previewCounts,
  resultCounts,
  routeImportError,
  usesMapping,
} from './import-data.js';

const FORM: ImportFormValue = { ...EMPTY_IMPORT_FORM, account: 'BofA Checking' };

const GENERIC: ImportFormValue = {
  ...FORM,
  format: GENERIC_FORMAT_CHOICE,
  mapping: { dateCol: 3, descCol: 1, amountCol: 4, dateFormat: '%d/%m/%Y' },
};

const FORMATS: ImporterFormat[] = [
  { key: 'bofa_checking', name: 'Bank of America Checking', accountTypes: ['checking'] },
];

const PROFILES: CsvProfile[] = [
  { name: 'chase', config: DEFAULT_CSV_MAPPING },
];

const PREVIEW: ImportPreview = {
  imported: 42,
  skipped: 3,
  malformed: 1,
  duplicateFile: false,
  sample: [],
  format: 'bofa_checking',
  importId: null,
};

const CONFIRMATION: ImportConfirmation = {
  ...PREVIEW,
  importId: 7,
  categorized: 38,
  stillFlagged: 6,
  snapshot: '/tmp/snap.db',
};

function apiError(
  code: string,
  status: number,
  message = 'nope',
  details?: unknown,
): ApiError {
  return new ApiError({
    code: code as ApiError['code'],
    rawCode: code,
    message,
    status,
    details,
  });
}

describe('the two mapping shapes', () => {
  it('are assignable both ways, so they cannot drift apart', () => {
    // `@nigel/ui` depends on lit alone and may not import api types, so the
    // four fields are declared twice. Mutual assignability is what makes a
    // field added to one and not the other a compile error rather than a bug
    // that only shows up as a 400 from the server.
    const uiToApi: GenericCsvConfig = DEFAULT_CSV_MAPPING;
    const apiToUi: GenericCsvMapping = uiToApi;

    expect(Object.keys(apiToUi).sort()).toEqual([
      'amountCol',
      'dateCol',
      'dateFormat',
      'descCol',
    ]);
  });
});

describe('importRequestBody', () => {
  it('sends neither format nor mapping when detecting', () => {
    expect(importRequestBody(FORM)).toEqual({ account: 'BofA Checking' });
  });

  it('sends the format alone for a built-in or a profile', () => {
    expect(importRequestBody({ ...FORM, format: 'bofa_checking' })).toEqual({
      account: 'BofA Checking',
      format: 'bofa_checking',
    });
    expect(importRequestBody({ ...FORM, format: 'chase' })).toEqual({
      account: 'BofA Checking',
      format: 'chase',
    });
  });

  it('sends the mapping alone for generic CSV', () => {
    const body = importRequestBody(GENERIC);
    expect(body).toEqual({
      account: 'BofA Checking',
      mapping: { dateCol: 3, descCol: 1, amountCol: 4, dateFormat: '%d/%m/%Y' },
    });
    expect(body).not.toHaveProperty('format');
  });

  it('never sends both, whatever the form holds', () => {
    // The API answers 400 for both together. Since the choice is one field,
    // there is no form value that can produce it.
    const candidates: ImportFormValue[] = [
      FORM,
      { ...FORM, format: 'bofa_checking' },
      GENERIC,
      { ...GENERIC, saveProfile: 'chase' },
    ];

    for (const form of candidates) {
      const body = importRequestBody(form);
      expect(
        body.format !== undefined && body.mapping !== undefined,
        JSON.stringify(form),
      ).toBe(false);
    }
  });

  it('copies the mapping rather than aliasing the form', () => {
    const body = importRequestBody(GENERIC);
    (body.mapping as GenericCsvConfig).amountCol = 99;
    expect(GENERIC.mapping.amountCol).toBe(4);
  });
});

describe('confirmRequestBody', () => {
  it('carries the profile name alongside a mapping', () => {
    const body = confirmRequestBody({ ...GENERIC, saveProfile: 'chase' });
    expect(body.saveProfile).toBe('chase');
    expect(body.mapping).toBeDefined();
  });

  it('trims the profile name', () => {
    expect(confirmRequestBody({ ...GENERIC, saveProfile: '  chase  ' }).saveProfile).toBe(
      'chase',
    );
  });

  it('omits an empty or whitespace-only profile name', () => {
    expect(confirmRequestBody({ ...GENERIC, saveProfile: '   ' })).not.toHaveProperty(
      'saveProfile',
    );
  });

  it('drops a profile name with no mapping to save', () => {
    // `saveProfile` without `mapping` is a 400; the pairing is enforced here
    // rather than discovered on the way back.
    const body = confirmRequestBody({
      ...FORM,
      format: 'bofa_checking',
      saveProfile: 'chase',
    });
    expect(body).not.toHaveProperty('saveProfile');
  });
});

describe('usesMapping', () => {
  it('is true only for the generic choice', () => {
    expect(usesMapping(GENERIC)).toBe(true);
    expect(usesMapping(FORM)).toBe(false);
    expect(usesMapping({ ...FORM, format: 'chase' })).toBe(false);
  });
});

describe('formatLabel', () => {
  it('names a built-in importer', () => {
    expect(formatLabel('bofa_checking', FORMATS, PROFILES)).toBe(
      'Bank of America Checking',
    );
  });

  it('names a saved profile as one', () => {
    expect(formatLabel('chase', FORMATS, PROFILES)).toBe('Saved profile: chase');
  });

  it('spells out the generic reader', () => {
    expect(formatLabel(GENERIC_FORMAT, FORMATS, PROFILES)).toContain('Generic CSV');
  });

  it('falls back to the raw key it does not recognize', () => {
    expect(formatLabel('something_new', FORMATS, PROFILES)).toBe('something_new');
  });

  it('handles the null a duplicate file reports', () => {
    expect(formatLabel(null, FORMATS, PROFILES)).toBe('Not determined');
  });
});

describe('counts', () => {
  it('reports the preview as to-import, duplicates and malformed', () => {
    expect(previewCounts(PREVIEW).map((c) => [c.label, c.value])).toEqual([
      ['To import', 42],
      ['Duplicates', 3],
      ['Malformed', 1],
    ]);
  });

  it('warns only when there are malformed rows', () => {
    expect(previewCounts(PREVIEW)[2].emphasis).toBe('warn');
    expect(previewCounts({ ...PREVIEW, malformed: 0 })[2].emphasis).toBe('default');
  });

  it('reports the result including categorization', () => {
    expect(resultCounts(CONFIRMATION).map((c) => [c.label, c.value])).toEqual([
      ['Imported', 42],
      ['Duplicates', 3],
      ['Malformed', 1],
      ['Categorized', 38],
      ['Still flagged', 6],
    ]);
  });

  it('says the flagged count is ledger-wide', () => {
    // It is not this import's flagged count, and a label that implied it was
    // would misread every time an older flagged row is still sitting there.
    expect(resultCounts(CONFIRMATION).at(-1)?.hint).toBe('across the ledger');
  });

  it('says the same of the categorized count', () => {
    // `categorize_transactions` scans every uncategorized transaction there
    // is, so the two counts have exactly the same scope.
    const counts = resultCounts(CONFIRMATION);
    expect(counts.find((count) => count.label === 'Categorized')?.hint).toBe(
      'across the ledger',
    );
  });
});

describe('routeImportError', () => {
  it('puts an oversize file under the dropzone', () => {
    const routed = routeImportError(apiError('payload_too_large', 413, 'too big'), FORM);
    expect(routed).toEqual({ field: 'dropzone', message: 'too big', toast: false });
  });

  it('puts a missing cargo feature under the format select', () => {
    const routed = routeImportError(
      apiError('feature_disabled', 501, 'no gusto here'),
      FORM,
    );
    expect(routed.field).toBe('format');
    expect(routed.message).toBe('no gusto here');
  });

  it('puts a bad mapping under the mapping form', () => {
    const routed = routeImportError(apiError('bad_request', 400, 'column 9'), GENERIC);
    expect(routed).toEqual({ field: 'mapping', message: 'column 9', toast: false });
  });

  it('puts the same 400 under the format select when there is no mapping', () => {
    expect(routeImportError(apiError('bad_request', 400), FORM).field).toBe('format');
  });

  it('puts an unknown account under the account select, and toasts it', () => {
    const routed = routeImportError(apiError('not_found', 404, 'no account'), FORM);
    expect(routed.field).toBe('account');
    expect(routed.toast).toBe(true);
  });

  it('treats an expired upload as a dropzone problem, not a missing account', () => {
    const routed = routeImportError(
      apiError('not_found', 404, 'gone', { reason: UPLOAD_NOT_FOUND }),
      FORM,
    );
    expect(routed.field).toBe('dropzone');
    expect(routed.message).toContain('expired');
  });

  it('says nothing about a locked or expired session', () => {
    // The shell gates both before a screen exists; a second telling here would
    // be a worse version of a story already on screen.
    for (const error of [apiError('locked', 423), apiError('unauthorized', 401)]) {
      const routed = routeImportError(error, FORM);
      expect(routed.field).toBe('none');
      expect(routed.toast).toBe(false);
    }
  });

  it('toasts anything with no obvious home', () => {
    const routed = routeImportError(apiError('internal', 500, 'boom'), FORM);
    expect(routed).toEqual({ field: 'none', message: 'boom', toast: true });
  });

  it('toasts a non-ApiError without pretending to know the message', () => {
    const routed = routeImportError(new Error('kaboom'), FORM);
    expect(routed.field).toBe('none');
    expect(routed.toast).toBe(true);
    expect(routed.message).not.toContain('kaboom');
  });
});
