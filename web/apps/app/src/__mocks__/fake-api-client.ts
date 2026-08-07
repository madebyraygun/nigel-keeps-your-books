import type { ApiClient } from '../api/client.js';
import type { PingResponse, StatusResponse, UnlockResponse } from '../api/types.js';

export const UNLOCKED_STATUS: StatusResponse = {
  initialized: true,
  encrypted: false,
  locked: false,
  companyName: 'Test Consultancy',
  version: '0.0.0-test',
  dataDir: '/tmp/nigel',
};

export const LOCKED_STATUS: StatusResponse = {
  ...UNLOCKED_STATUS,
  encrypted: true,
  locked: true,
  companyName: null,
};

/**
 * An ApiClient that never touches the network.
 *
 * Its existence is the point: if a screen can be driven by this, the api
 * interface really is a seam, and a Tauri or remote-server implementation can
 * take the same place without a component noticing.
 */
export class FakeApiClient implements ApiClient {
  status: StatusResponse = UNLOCKED_STATUS;
  statusError: Error | null = null;
  unlockError: Error | null = null;

  readonly calls: string[] = [];

  async ping(): Promise<PingResponse> {
    this.calls.push('ping');
    return { ok: true, version: this.status.version };
  }

  async getStatus(): Promise<StatusResponse> {
    this.calls.push('getStatus');
    if (this.statusError) throw this.statusError;
    return this.status;
  }

  async unlock(password: string): Promise<UnlockResponse> {
    this.calls.push(`unlock:${password}`);
    if (this.unlockError) throw this.unlockError;
    this.status = { ...this.status, locked: false };
    return { locked: false };
  }
}
