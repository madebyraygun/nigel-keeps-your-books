import {
  GENERIC_FORMAT_CHOICE,
  type CountItem,
  type ImportFormValue,
} from '@nigel/ui';

import { ApiError } from '../api/index.js';
import {
  GENERIC_FORMAT,
  type ConfirmImportRequest,
  type CsvProfile,
  type ImportConfirmation,
  type ImporterFormat,
  type ImportPreview,
  type ImportRequest,
} from '../api/types.js';

/** A request body without the upload it applies to. */
export type ImportBody = Omit<ImportRequest, 'uploadId'>;
export type ConfirmBody = Omit<ConfirmImportRequest, 'uploadId'>;

/**
 * The format/mapping half of a preview or confirm body.
 *
 * The API refuses `format` and `mapping` together. Deriving both from one
 * form field is what makes that unrepresentable here rather than merely
 * avoided: there is no branch in which both are set.
 */
export function importRequestBody(form: ImportFormValue): ImportBody {
  if (form.format === GENERIC_FORMAT_CHOICE) {
    return { account: form.account, mapping: { ...form.mapping } };
  }
  if (form.format === '') {
    return { account: form.account };
  }
  return { account: form.account, format: form.format };
}

/**
 * The same body plus a profile name, when there is a mapping to save under it.
 *
 * `saveProfile` without a `mapping` is a 400, so the pairing is enforced on the
 * way out rather than discovered on the way back.
 */
export function confirmRequestBody(form: ImportFormValue): ConfirmBody {
  const body = importRequestBody(form);
  const name = form.saveProfile.trim();
  if (body.mapping === undefined || name === '') return body;
  return { ...body, saveProfile: name };
}

/** Whether this form would send an inline mapping — i.e. whose 400 it is. */
export function usesMapping(form: ImportFormValue): boolean {
  return form.format === GENERIC_FORMAT_CHOICE;
}

/** How the effective format should read to someone who did not choose it. */
export function formatLabel(
  format: string | null,
  formats: ImporterFormat[],
  profiles: CsvProfile[],
): string {
  if (format === null) return 'Not determined';
  if (format === GENERIC_FORMAT) return 'Generic CSV (your column mapping)';

  const builtIn = formats.find((candidate) => candidate.key === format);
  if (builtIn) return builtIn.name;

  const profile = profiles.find((candidate) => candidate.name === format);
  if (profile) return `Saved profile: ${profile.name}`;

  return format;
}

/** What the dry run says would happen. */
export function previewCounts(preview: ImportPreview): CountItem[] {
  return [
    { label: 'To import', value: preview.imported },
    { label: 'Duplicates', value: preview.skipped },
    {
      label: 'Malformed',
      value: preview.malformed,
      emphasis: preview.malformed > 0 ? 'warn' : 'default',
    },
  ];
}

/** What the import actually did. */
export function resultCounts(result: ImportConfirmation): CountItem[] {
  return [
    { label: 'Imported', value: result.imported, emphasis: 'good' },
    { label: 'Duplicates', value: result.skipped },
    {
      label: 'Malformed',
      value: result.malformed,
      emphasis: result.malformed > 0 ? 'warn' : 'default',
    },
    { label: 'Categorized', value: result.categorized, emphasis: 'good' },
    {
      label: 'Still flagged',
      value: result.stillFlagged,
      emphasis: result.stillFlagged > 0 ? 'warn' : 'default',
      hint: 'across the ledger',
    },
  ];
}

/** Where a failure belongs on the screen. */
export type ErrorField = 'dropzone' | 'account' | 'format' | 'mapping' | 'none';

export interface RoutedError {
  field: ErrorField;
  message: string;
  /** Whether it also deserves a toast. */
  toast: boolean;
}

const GENERIC_MESSAGE = 'Something went wrong with that import.';

/**
 * Put a failure where the thing that caused it is.
 *
 * A 400 about columns belongs under the column fields, not in a toast that
 * disappears while you are still reading the form. Anything with no obvious
 * home is a toast, and 423/401 are neither: the shell gates those before a
 * screen is ever constructed, so repeating them here would be a second,
 * worse version of a story already being told.
 */
export function routeImportError(error: unknown, form: ImportFormValue): RoutedError {
  if (!(error instanceof ApiError)) {
    return { field: 'none', message: GENERIC_MESSAGE, toast: true };
  }

  if (error.isLocked || error.isUnauthorized) {
    return { field: 'none', message: error.message, toast: false };
  }

  if (error.isUploadExpired) {
    return {
      field: 'dropzone',
      message: 'That upload expired. Choose the file again.',
      toast: false,
    };
  }

  switch (error.code) {
    case 'payload_too_large':
      return { field: 'dropzone', message: error.message, toast: false };
    case 'feature_disabled':
      return { field: 'format', message: error.message, toast: false };
    case 'not_found':
      return { field: 'account', message: error.message, toast: true };
    case 'bad_request':
      return {
        field: usesMapping(form) ? 'mapping' : 'format',
        message: error.message,
        toast: false,
      };
    default:
      return { field: 'none', message: error.message, toast: true };
  }
}
