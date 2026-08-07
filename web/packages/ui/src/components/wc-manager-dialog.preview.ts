import { html } from 'lit';
import './wc-manager-dialog.js';
import './wc-account-form.js';
import type { Preview } from '../../preview/types.js';

const value = {
  name: 'Chase Business',
  accountType: 'checking',
  institution: 'Chase',
  lastFour: '9921',
};

const preview: Preview = {
  id: 'wc-manager-dialog',
  title: 'Manager dialog',
  group: 'Overlays',
  description:
    'The frame around a manager form. A save that comes back 409 renders here, beside the field that caused it, rather than in a toast that leaves before it is read.',
  layout: 'stack',
  states: [
    {
      name: 'closed',
      render: () => html`<wc-manager-dialog heading="Add account"></wc-manager-dialog>`,
    },
    {
      name: 'default',
      render: () => html`
        <wc-manager-dialog open heading="Add account" confirm-label="Add">
          <wc-account-form .value=${value}></wc-account-form>
        </wc-manager-dialog>
      `,
    },
    {
      name: 'edit',
      render: () => html`
        <wc-manager-dialog open heading="Rename account">
          <wc-account-form mode="rename" .value=${value}></wc-account-form>
        </wc-manager-dialog>
      `,
    },
    {
      name: 'with-error',
      render: () => html`
        <wc-manager-dialog
          open
          heading="Add account"
          confirm-label="Add"
          error="An account named “Chase Business” already exists."
        >
          <wc-account-form .value=${value}></wc-account-form>
        </wc-manager-dialog>
      `,
    },
    {
      name: 'busy',
      render: () => html`
        <wc-manager-dialog open busy heading="Add account" confirm-label="Add">
          <wc-account-form .value=${value} disabled></wc-account-form>
        </wc-manager-dialog>
      `,
    },
  ],
};

export default preview;
