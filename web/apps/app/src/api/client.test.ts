import { describe, it, expect, beforeEach, vi } from 'vitest';
import { ApiError, FetchApiClient, appLocked, appUnauthorized } from './client.js';

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json' },
  });
}

function envelope(code: string, message: string, details?: unknown, status = 400): Response {
  return jsonResponse({ error: { code, message, ...(details ? { details } : {}) } }, status);
}

function clientFor(fetchImpl: typeof fetch): FetchApiClient {
  return new FetchApiClient({ fetchImpl });
}

describe('FetchApiClient', () => {
  beforeEach(() => {
    appLocked.set(false);
    appUnauthorized.set(false);
  });

  describe('requests', () => {
    it('calls /api by default and sends the session cookie', async () => {
      const fetchImpl = vi.fn().mockResolvedValue(jsonResponse({ ok: true, version: '1' }));
      await clientFor(fetchImpl).ping();

      const [url, init] = fetchImpl.mock.calls[0];
      expect(url).toBe('/api/ping');
      expect(init.credentials).toBe('same-origin');
      expect(init.method).toBe('GET');
    });

    it('honours a custom base url', async () => {
      const fetchImpl = vi.fn().mockResolvedValue(jsonResponse({ ok: true, version: '1' }));
      await new FetchApiClient({ fetchImpl, baseUrl: 'http://127.0.0.1:5731/api' }).ping();
      expect(fetchImpl.mock.calls[0][0]).toBe('http://127.0.0.1:5731/api/ping');
    });

    it('posts the password as a JSON body on unlock', async () => {
      const fetchImpl = vi.fn().mockResolvedValue(jsonResponse({ locked: false }));
      await clientFor(fetchImpl).unlock('hunter2');

      const [url, init] = fetchImpl.mock.calls[0];
      expect(url).toBe('/api/unlock');
      expect(init.method).toBe('POST');
      expect(JSON.parse(init.body)).toEqual({ password: 'hunter2' });
    });

    it('returns the parsed body', async () => {
      const status = {
        initialized: true,
        encrypted: false,
        locked: false,
        companyName: 'Raygun',
        version: '0.1.0',
        dataDir: '/home/x/Documents/nigel',
      };
      const result = await clientFor(
        vi.fn().mockResolvedValue(jsonResponse(status)),
      ).getStatus();
      expect(result).toEqual(status);
    });

    it('resolves undefined for an empty body', async () => {
      const fetchImpl = vi.fn().mockResolvedValue(new Response(null, { status: 204 }));
      await expect(clientFor(fetchImpl).ping()).resolves.toBeUndefined();
    });
  });

  describe('error normalization', () => {
    it('reads code, message and details out of the envelope', async () => {
      const fetchImpl = vi
        .fn()
        .mockResolvedValue(envelope('not_found', 'no such account', { id: 4 }, 404));

      const error = await clientFor(fetchImpl).getStatus().catch((e) => e);

      expect(error).toBeInstanceOf(ApiError);
      expect(error.code).toBe('not_found');
      expect(error.status).toBe(404);
      expect(error.message).toBe('no such account');
      expect(error.details).toEqual({ id: 4 });
    });

    it('keeps an unrecognised code as unknown without losing the original', async () => {
      // 31.7 introduces payload_too_large; a client built before it must not
      // crash or mistype the error when a newer server sends one.
      const fetchImpl = vi
        .fn()
        .mockResolvedValue(envelope('payload_too_large', 'too big', undefined, 413));

      const error = await clientFor(fetchImpl).getStatus().catch((e) => e);

      expect(error.code).toBe('unknown');
      expect(error.rawCode).toBe('payload_too_large');
      expect(error.status).toBe(413);
    });

    it('derives a code from the status when the body is not an envelope', async () => {
      const fetchImpl = vi
        .fn()
        .mockResolvedValue(new Response('<html>oops</html>', { status: 500 }));

      const error = await clientFor(fetchImpl).getStatus().catch((e) => e);

      expect(error.code).toBe('internal');
      expect(error.status).toBe(500);
    });

    it('reports a transport failure as status 0', async () => {
      const fetchImpl = vi.fn().mockRejectedValue(new TypeError('connection refused'));

      const error = await clientFor(fetchImpl).getStatus().catch((e) => e);

      expect(error).toBeInstanceOf(ApiError);
      expect(error.status).toBe(0);
      expect(error.message).toBe('Could not reach the nigel server.');
    });
  });

  describe('transport signals', () => {
    it('raises appLocked on a 423', async () => {
      const fetchImpl = vi
        .fn()
        .mockResolvedValue(envelope('locked', 'database is locked', undefined, 423));

      await clientFor(fetchImpl).getStatus().catch(() => {});

      expect(appLocked.get()).toBe(true);
    });

    it('raises appUnauthorized on a session 401', async () => {
      const fetchImpl = vi
        .fn()
        .mockResolvedValue(envelope('unauthorized', 'missing session', undefined, 401));

      await clientFor(fetchImpl).getStatus().catch(() => {});

      expect(appUnauthorized.get()).toBe(true);
    });

    it('leaves appUnauthorized alone for a wrong password', async () => {
      // invalid_password is also a 401. Treating it as a dead session would
      // throw the reauth banner in front of someone who just mistyped.
      const fetchImpl = vi.fn().mockResolvedValue(
        envelope(
          'invalid_password',
          'Wrong password.',
          { attemptsRemaining: 2, retryAfterMs: 1000 },
          401,
        ),
      );

      const error = await clientFor(fetchImpl).unlock('nope').catch((e) => e);

      expect(appUnauthorized.get()).toBe(false);
      expect(error.isUnauthorized).toBe(false);
      expect(error.invalidPasswordDetails()).toEqual({
        attemptsRemaining: 2,
        retryAfterMs: 1000,
      });
    });

    it('returns null details for a non-password error', async () => {
      const fetchImpl = vi
        .fn()
        .mockResolvedValue(envelope('unauthorized', 'nope', undefined, 401));
      const error = await clientFor(fetchImpl).getStatus().catch((e) => e);
      expect(error.invalidPasswordDetails()).toBeNull();
    });

    it('clears appUnauthorized after a successful call', async () => {
      appUnauthorized.set(true);
      const fetchImpl = vi.fn().mockResolvedValue(jsonResponse({ ok: true, version: '1' }));

      await clientFor(fetchImpl).ping();

      expect(appUnauthorized.get()).toBe(false);
    });

    it('lets getStatus drive appLocked in both directions', async () => {
      appLocked.set(true);
      const unlocked = {
        initialized: true,
        encrypted: true,
        locked: false,
        companyName: null,
        version: '0.1.0',
        dataDir: '/tmp',
      };

      await clientFor(vi.fn().mockResolvedValue(jsonResponse(unlocked))).getStatus();
      expect(appLocked.get()).toBe(false);

      await clientFor(
        vi.fn().mockResolvedValue(jsonResponse({ ...unlocked, locked: true })),
      ).getStatus();
      expect(appLocked.get()).toBe(true);
    });
  });
});
