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

/** A fetch that answers every POST with an empty object. */
function jsonPost() {
  return vi.fn().mockResolvedValue(jsonResponse({}));
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
      // A server newer than this client can name a code the client has never
      // heard of. That must not crash or be mistyped as one we do know — the
      // raw string stays reachable so a caller can still branch on it.
      const fetchImpl = vi
        .fn()
        .mockResolvedValue(envelope('teapot_overheated', 'too hot', undefined, 418));

      const error = await clientFor(fetchImpl).getStatus().catch((e) => e);

      expect(error.code).toBe('unknown');
      expect(error.rawCode).toBe('teapot_overheated');
      expect(error.status).toBe(418);
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

  describe('reports', () => {
    const wrapped = (report: unknown, granularity = 'monthAndYear') =>
      jsonResponse({ granularity, report });

    it('asks for a year of profit and loss', async () => {
      const fetchImpl = vi.fn().mockResolvedValue(wrapped({ net: 1 }));
      await clientFor(fetchImpl).getPnl({ year: 2026 });
      expect(fetchImpl.mock.calls[0][0]).toBe('/api/reports/pnl?year=2026');
    });

    it('omits parameters that were not given', async () => {
      // The server rejects a parameter a route does not support rather than
      // ignoring it, so a stray empty `month=` would be a 400.
      const fetchImpl = vi.fn().mockResolvedValue(wrapped({ net: 1 }));
      await clientFor(fetchImpl).getPnl();
      expect(fetchImpl.mock.calls[0][0]).toBe('/api/reports/pnl');
    });

    it('carries a from/to pair through', async () => {
      const fetchImpl = vi.fn().mockResolvedValue(wrapped({ net: 1 }));
      await clientFor(fetchImpl).getPnl({ from: '2026-01-01', to: '2026-03-31' });
      expect(fetchImpl.mock.calls[0][0]).toBe(
        '/api/reports/pnl?from=2026-01-01&to=2026-03-31',
      );
    });

    it('unwraps the envelope rather than handing screens the wrapper', async () => {
      const report = {
        income: [],
        expenses: [],
        totalIncome: 0,
        totalExpenses: 0,
        net: 0,
      };
      const fetchImpl = vi.fn().mockResolvedValue(wrapped(report));
      const answer = await clientFor(fetchImpl).getPnl({ year: 2026 });
      expect(answer.report).toEqual(report);
      expect(answer.granularity).toBe('monthAndYear');
    });

    it('takes no parameters for balance', async () => {
      const fetchImpl = vi
        .fn()
        .mockResolvedValue(
          wrapped({ accounts: [], total: 0, ytdNetIncome: 0 }, 'none'),
        );
      await clientFor(fetchImpl).getBalance();
      expect(fetchImpl.mock.calls[0][0]).toBe('/api/reports/balance');
    });

    it('asks for cash flow unfiltered by default', async () => {
      const fetchImpl = vi.fn().mockResolvedValue(wrapped({ months: [] }));
      await clientFor(fetchImpl).getCashflow();
      expect(fetchImpl.mock.calls[0][0]).toBe('/api/reports/cashflow');
    });

    it('can narrow cash flow to a year', async () => {
      const fetchImpl = vi.fn().mockResolvedValue(wrapped({ months: [] }));
      await clientFor(fetchImpl).getCashflow({ year: 2025 });
      expect(fetchImpl.mock.calls[0][0]).toBe('/api/reports/cashflow?year=2025');
    });

    it('reads the flagged list out of its envelope', async () => {
      const rows = [
        { id: 1, date: '2026-01-01', description: 'X', amount: -5, accountName: 'A' },
      ];
      const fetchImpl = vi.fn().mockResolvedValue(wrapped(rows, 'none'));
      const answer = await clientFor(fetchImpl).getFlagged();
      expect(fetchImpl.mock.calls[0][0]).toBe('/api/reports/flagged');
      expect(answer.report).toEqual(rows);
    });

    it('raises the lock signal when a report is refused', async () => {
      const fetchImpl = vi
        .fn()
        .mockResolvedValue(
          envelope('locked', 'Database is locked.', undefined, 423),
        );
      await expect(clientFor(fetchImpl).getBalance()).rejects.toBeInstanceOf(
        ApiError,
      );
      expect(appLocked.get()).toBe(true);
    });
  });

  describe('imports', () => {
    it('uploads the file as multipart, letting the browser set the boundary', async () => {
      const fetchImpl = vi
        .fn()
        .mockResolvedValue(jsonResponse({ uploadId: 'a1', filename: 'x.csv', size: 9 }));
      const file = new File(['date,desc,amount'], 'april.csv');

      const answer = await clientFor(fetchImpl).uploadImport(file);

      const [url, init] = fetchImpl.mock.calls[0];
      expect(url).toBe('/api/imports/upload');
      expect(init.method).toBe('POST');
      expect(init.body).toBeInstanceOf(FormData);
      // jsdom's FormData re-wraps the blob, so identity is not preserved;
      // what matters is that the name the server records survives.
      const sent = (init.body as FormData).get('file') as File;
      expect(sent.name).toBe('april.csv');
      // Naming the content type here would omit the multipart boundary, which
      // only the browser can generate, and the server could not parse the body.
      expect(init.headers).toBeUndefined();
      expect(answer.uploadId).toBe('a1');
    });

    it('still sends the JSON content type for every other call', async () => {
      const fetchImpl = jsonPost();
      await clientFor(fetchImpl).previewImport({ uploadId: 'a1', account: 'Checking' });

      const [url, init] = fetchImpl.mock.calls[0];
      expect(url).toBe('/api/imports/preview');
      expect(init.headers).toEqual({ 'Content-Type': 'application/json' });
      expect(JSON.parse(init.body)).toEqual({ uploadId: 'a1', account: 'Checking' });
    });

    it('posts the confirm body including a profile to save', async () => {
      const fetchImpl = jsonPost();
      await clientFor(fetchImpl).confirmImport({
        uploadId: 'a1',
        account: 'Checking',
        mapping: { dateCol: 0, descCol: 1, amountCol: 2, dateFormat: '%m/%d/%Y' },
        saveProfile: 'chase',
      });

      expect(fetchImpl.mock.calls[0][0]).toBe('/api/imports/confirm');
      expect(JSON.parse(fetchImpl.mock.calls[0][1].body).saveProfile).toBe('chase');
    });

    it('reads the format and profile lists', async () => {
      const formats = [{ key: 'bofa_checking', name: 'BofA', accountTypes: ['checking'] }];
      const fetchImpl = vi.fn().mockResolvedValue(jsonResponse(formats));
      expect(await clientFor(fetchImpl).getImportFormats()).toEqual(formats);
      expect(fetchImpl.mock.calls[0][0]).toBe('/api/imports/formats');

      const profiles = vi.fn().mockResolvedValue(jsonResponse([]));
      await clientFor(profiles).getCsvProfiles();
      expect(profiles.mock.calls[0][0]).toBe('/api/csv-profiles');
    });

    it('normalizes a 413 to payload_too_large', async () => {
      const fetchImpl = vi
        .fn()
        .mockResolvedValue(envelope('payload_too_large', 'Too big.', undefined, 413));

      const error = await clientFor(fetchImpl)
        .uploadImport(new File([''], 'x.csv'))
        .catch((e: unknown) => e as ApiError);

      expect(error).toBeInstanceOf(ApiError);
      expect((error as ApiError).code).toBe('payload_too_large');
      expect((error as ApiError).status).toBe(413);
    });

    it('recognizes an expired upload among other 404s', async () => {
      const expired = new ApiError({
        code: 'not_found',
        rawCode: 'not_found',
        message: 'gone',
        status: 404,
        details: { reason: 'upload_not_found' },
      });
      expect(expired.isUploadExpired).toBe(true);

      const missingAccount = new ApiError({
        code: 'not_found',
        rawCode: 'not_found',
        message: 'no account',
        status: 404,
      });
      expect(missingAccount.isUploadExpired).toBe(false);

      const otherStatus = new ApiError({
        code: 'bad_request',
        rawCode: 'bad_request',
        message: 'no',
        status: 400,
        details: { reason: 'upload_not_found' },
      });
      expect(otherStatus.isUploadExpired).toBe(false);
    });
  });
});
