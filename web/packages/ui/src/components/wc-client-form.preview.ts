import { html } from 'lit';
import './wc-client-form.js';
import { EMPTY_CLIENT_FORM } from './wc-client-form.js';
import type { Preview } from '../../preview/types.js';

const filled = {
  name: 'Acme Co',
  email: 'ap@acme.test',
  billingAddress: '1 Main St, Springfield',
  notes: 'Net 30, PO required',
};

const preview: Preview = {
  id: 'wc-client-form',
  title: 'Client form',
  group: 'Invoicing',
  description:
    'Name, email, billing address and notes. The email is not shape-checked, because `nigel client add` does not check it either — but a client with none is told what it costs.',
  layout: 'stack',
  states: [
    {
      name: 'add',
      render: () => html`<wc-client-form .value=${EMPTY_CLIENT_FORM}></wc-client-form>`,
    },
    { name: 'edit', render: () => html`<wc-client-form .value=${filled}></wc-client-form>` },
    {
      name: 'missing-email',
      render: () => html`<wc-client-form .value=${{ ...filled, email: '' }}></wc-client-form>`,
    },
    {
      name: 'duplicate-name',
      render: () => html`
        <wc-client-form
          .value=${{ ...filled, name: '' }}
          .errors=${{ name: 'Name is required' }}
        ></wc-client-form>
      `,
    },
    {
      name: 'saving',
      render: () => html`<wc-client-form .value=${filled} disabled></wc-client-form>`,
    },
  ],
};

export default preview;
