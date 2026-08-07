import { describe, it, expect } from 'vitest';
import { parseRoute, routeToUrl } from './router.js';

describe('parseRoute', () => {
  it('reads preview, state and mode', () => {
    expect(
      parseRoute('http://localhost:9090/?preview=wc-money&state=negative&mode=preview'),
    ).toEqual({ previewId: 'wc-money', stateName: 'negative', mode: 'preview' });
  });

  it('omits absent parameters', () => {
    expect(parseRoute('http://localhost:9090/')).toEqual({});
  });

  it('ignores an unrecognised mode', () => {
    expect(parseRoute('http://localhost:9090/?mode=nonsense').mode).toBeUndefined();
  });
});

describe('routeToUrl', () => {
  it('serialises a full route', () => {
    expect(routeToUrl({ previewId: 'wc-money', stateName: 'negative', mode: 'preview' })).toBe(
      '?preview=wc-money&state=negative&mode=preview',
    );
  });

  it('returns the root for an empty route', () => {
    expect(routeToUrl({})).toBe('/');
  });

  it('round-trips', () => {
    const route = { previewId: 'wc-toast', stateName: 'danger' };
    expect(parseRoute(`http://localhost:9090/${routeToUrl(route)}`)).toEqual(route);
  });
});
