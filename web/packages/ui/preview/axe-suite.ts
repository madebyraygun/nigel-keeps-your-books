import { describe, it, expect, afterEach } from 'vitest';
import { render } from 'lit';
import { runA11y } from './a11y.js';
import type { Preview } from './types.js';

/**
 * Assert zero axe violations for **every** state a preview declares.
 *
 * Step 3 of the component-first workflow says each component's states pass
 * axe. Spelling the states out again inside the test file is how that promise
 * rots: a state added to the preview is silently never checked. Driving the
 * suite off the preview object means adding a state adds its a11y test.
 */
export function describePreviewA11y(preview: Preview): void {
  describe(`${preview.title} a11y`, () => {
    let host: HTMLDivElement | null = null;

    afterEach(() => {
      host?.remove();
      host = null;
    });

    it('declares at least one state', () => {
      expect(preview.states.length).toBeGreaterThan(0);
    });

    it.each(preview.states.map((s) => [s.name, s] as const))(
      'state %s has no axe violations',
      async (_name, state) => {
        host = document.createElement('div');
        document.body.appendChild(host);
        render(state.render(), host);

        // Let the custom elements in the template finish their first update.
        await new Promise((resolve) => setTimeout(resolve, 0));

        const { violations } = await runA11y(host);
        expect(
          violations.map((v) => `${v.id}: ${v.help}`),
          `axe violations in "${preview.id}" state "${state.name}"`,
        ).toEqual([]);
      },
    );
  });
}
