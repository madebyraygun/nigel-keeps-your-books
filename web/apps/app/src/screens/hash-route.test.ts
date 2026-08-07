import { describe, it, expect } from 'vitest';
import { parseHash, routeToHash, screenToHash } from './hash-route.js';

describe('parseHash', () => {
  it('reads a screen id', () => {
    expect(parseHash('#/register').screen).toBe('register');
  });

  it('tolerates a missing slash', () => {
    expect(parseHash('#register').screen).toBe('register');
  });

  it('falls back to the dashboard for an empty hash', () => {
    expect(parseHash('').screen).toBe('dashboard');
    expect(parseHash('#').screen).toBe('dashboard');
    expect(parseHash('#/').screen).toBe('dashboard');
  });

  it('falls back to the dashboard for an unknown screen', () => {
    // A stale bookmark should land somewhere useful, not on a blank page.
    expect(parseHash('#/nonsense').screen).toBe('dashboard');
  });

  it('preserves query parameters for the screen tasks', () => {
    const route = parseHash('#/register?year=2025&account=BofA%20Checking');
    expect(route.screen).toBe('register');
    expect(route.params.get('year')).toBe('2025');
    expect(route.params.get('account')).toBe('BofA Checking');
  });

  it('yields empty params when there is no query', () => {
    expect([...parseHash('#/register').params]).toEqual([]);
  });

  it('does not treat a param-only hash as a screen', () => {
    expect(parseHash('#/?year=2025').screen).toBe('dashboard');
  });
});

describe('routeToHash', () => {
  it('serialises a bare screen', () => {
    expect(routeToHash({ screen: 'reports', params: new URLSearchParams() })).toBe(
      '#/reports',
    );
  });

  it('appends params when present', () => {
    expect(
      routeToHash({ screen: 'register', params: new URLSearchParams({ year: '2025' }) }),
    ).toBe('#/register?year=2025');
  });

  it('round-trips', () => {
    const hash = '#/register?year=2025';
    expect(routeToHash(parseHash(hash))).toBe(hash);
  });
});

describe('screenToHash', () => {
  it('builds the hash a nav click writes', () => {
    expect(screenToHash('undo')).toBe('#/undo');
  });
});
