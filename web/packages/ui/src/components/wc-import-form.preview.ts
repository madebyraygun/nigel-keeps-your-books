import { html } from 'lit';
import './wc-import-form.js';
import type { Preview } from '../../preview/types.js';
import {
  DEFAULT_CSV_MAPPING,
  EMPTY_IMPORT_FORM,
  GENERIC_FORMAT_CHOICE,
  type ImportAccountOption,
  type ImportFormatOption,
} from './wc-import-form.js';

const ACCOUNTS: ImportAccountOption[] = [
  { id: 1, name: 'BofA Checking', accountType: 'checking' },
  { id: 2, name: 'BofA Credit Card', accountType: 'credit_card' },
];

const FORMATS: ImportFormatOption[] = [
  { key: 'bofa_checking', name: 'Bank of America Checking', accountTypes: ['checking'] },
  {
    key: 'bofa_credit_card',
    name: 'Bank of America Credit Card',
    accountTypes: ['credit_card'],
  },
];

const CHOSEN = { ...EMPTY_IMPORT_FORM, account: 'BofA Checking' };

const preview: Preview = {
  id: 'wc-import-form',
  title: 'Import Form',
  group: 'Forms',
  description: 'Account, format override, and the generic CSV column mapping.',
  layout: 'stack',
  states: [
    {
      name: 'default',
      render: () =>
        html`<wc-import-form
          .accounts=${ACCOUNTS}
          .formats=${FORMATS}
          .value=${CHOSEN}
        ></wc-import-form>`,
    },
    {
      name: 'with-profiles',
      render: () =>
        html`<wc-import-form
          .accounts=${ACCOUNTS}
          .formats=${FORMATS}
          .profiles=${['chase', 'wells-fargo']}
          .value=${CHOSEN}
        ></wc-import-form>`,
    },
    {
      name: 'generic-open',
      render: () =>
        html`<wc-import-form
          .accounts=${ACCOUNTS}
          .formats=${FORMATS}
          .value=${{
            ...CHOSEN,
            format: GENERIC_FORMAT_CHOICE,
            mapping: DEFAULT_CSV_MAPPING,
          }}
        ></wc-import-form>`,
    },
    {
      name: 'mapping-error',
      render: () =>
        html`<wc-import-form
          .accounts=${ACCOUNTS}
          .formats=${FORMATS}
          .value=${{
            ...CHOSEN,
            format: GENERIC_FORMAT_CHOICE,
            mapping: { ...DEFAULT_CSV_MAPPING, amountCol: 9 },
          }}
          mapping-error="Column 9 is past the end of every row in this file."
        ></wc-import-form>`,
    },
    {
      name: 'account-error',
      render: () =>
        html`<wc-import-form
          .accounts=${ACCOUNTS}
          .formats=${FORMATS}
          .value=${CHOSEN}
          account-error="That account is not there any more."
        ></wc-import-form>`,
    },
    {
      name: 'format-error',
      render: () =>
        html`<wc-import-form
          .accounts=${ACCOUNTS}
          .formats=${FORMATS}
          .value=${{ ...CHOSEN, format: 'gusto_payroll' }}
          format-error="This build has no Gusto payroll support."
        ></wc-import-form>`,
    },
    {
      name: 'no-accounts',
      render: () => html`<wc-import-form .formats=${FORMATS}></wc-import-form>`,
    },
    {
      name: 'disabled',
      render: () =>
        html`<wc-import-form
          disabled
          .accounts=${ACCOUNTS}
          .formats=${FORMATS}
          .value=${CHOSEN}
        ></wc-import-form>`,
    },
  ],
};

export default preview;
