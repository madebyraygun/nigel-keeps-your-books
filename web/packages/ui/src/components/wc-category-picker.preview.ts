import { html } from 'lit';
import './wc-category-picker.js';
import type { Preview } from '../../preview/types.js';
import type { CategoryOption } from './category-option.js';

const categories: CategoryOption[] = [
  { id: 3, name: 'Consulting income', categoryType: 'income' },
  { id: 4, name: 'Interest income', categoryType: 'income' },
  { id: 12, name: 'Software / Subscriptions', categoryType: 'expense' },
  { id: 13, name: 'Meals', categoryType: 'expense' },
  { id: 14, name: 'Rent / Lease', categoryType: 'expense' },
];

const preview: Preview = {
  id: 'wc-category-picker',
  title: 'Category Picker',
  group: 'Data',
  description:
    'Searchable chart-of-accounts combobox, grouped income then expense. Shared by the register editor and the review form.',
  layout: 'stack',
  states: [
    {
      name: 'closed',
      render: () =>
        html`<wc-category-picker .options=${categories}></wc-category-picker>`,
    },
    {
      name: 'selected',
      render: () =>
        html`<wc-category-picker
          .options=${categories}
          .value=${12}
        ></wc-category-picker>`,
    },
    {
      name: 'open',
      render: () =>
        html`<wc-category-picker
          .options=${categories}
          .listOpen=${true}
        ></wc-category-picker>`,
    },
    {
      name: 'filtered',
      render: () =>
        html`<wc-category-picker
          .options=${categories}
          .listOpen=${true}
          .queryText=${'inc'}
        ></wc-category-picker>`,
    },
    {
      name: 'no-matches',
      render: () =>
        html`<wc-category-picker
          .options=${categories}
          .listOpen=${true}
          .queryText=${'zzz'}
        ></wc-category-picker>`,
    },
    {
      name: 'invalid',
      render: () =>
        html`<wc-category-picker .options=${categories} invalid></wc-category-picker>`,
    },
    {
      name: 'disabled',
      render: () =>
        html`<wc-category-picker .options=${categories} disabled></wc-category-picker>`,
    },
  ],
};

export default preview;
