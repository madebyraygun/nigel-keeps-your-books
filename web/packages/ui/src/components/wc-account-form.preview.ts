import { html } from 'lit';
import './wc-account-form.js';
import { EMPTY_ACCOUNT_FORM } from './wc-account-form.js';
import type { Preview } from '../../preview/types.js';

const filled = {
  name: 'BofA Checking',
  accountType: 'checking',
  institution: 'Bank of America',
  lastFour: '4821',
};

const preview: Preview = {
  id: 'wc-account-form',
  title: 'Account form',
  group: 'Data',
  description:
    'Add collects all four fields; rename collects the one the PATCH route accepts and shows the rest as the creation-time facts they are.',
  layout: 'stack',
  states: [
    {
      name: 'create',
      render: () => html`<wc-account-form .value=${EMPTY_ACCOUNT_FORM}></wc-account-form>`,
    },
    {
      name: 'rename',
      render: () => html`<wc-account-form mode="rename" .value=${filled}></wc-account-form>`,
    },
    {
      name: 'with-error',
      render: () => html`
        <wc-account-form
          .value=${{ ...filled, name: '', lastFour: '12a' }}
          .errors=${{
            name: 'Name is required',
            lastFour: 'Last four must be exactly 4 digits',
          }}
        ></wc-account-form>
      `,
    },
    {
      name: 'disabled',
      render: () => html`<wc-account-form .value=${filled} disabled></wc-account-form>`,
    },
  ],
};

export default preview;
