import { describe, it, expect } from 'vitest';
import { html } from 'lit';
import { collectPreviews } from './manifest.js';
import type { Preview } from './types.js';

function preview(id: string, group: string, title: string): Preview {
  return { id, group, title, states: [{ name: 'default', render: () => html`<i></i>` }] };
}

describe('collectPreviews', () => {
  it('unwraps the default export of each module', () => {
    const result = collectPreviews({
      './a.preview.ts': { default: preview('a', 'Layout', 'A') },
    });
    expect(result.map((p) => p.id)).toEqual(['a']);
  });

  it('sorts by group then title', () => {
    const result = collectPreviews({
      './c.preview.ts': { default: preview('c', 'Layout', 'Zebra') },
      './a.preview.ts': { default: preview('a', 'Data', 'Money') },
      './b.preview.ts': { default: preview('b', 'Layout', 'Alpha') },
    });
    expect(result.map((p) => p.id)).toEqual(['a', 'b', 'c']);
  });

  it('returns an empty list for no modules', () => {
    expect(collectPreviews({})).toEqual([]);
  });
});
