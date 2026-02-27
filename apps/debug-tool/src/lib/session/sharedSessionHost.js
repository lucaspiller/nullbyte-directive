const DEFAULT_BRIDGE_URL = 'http://127.0.0.1:48765';
const SNAPSHOT_BEFORE = 3;
const SNAPSHOT_AFTER = 4;
const POLL_INTERVAL_MS = 250;

let sharedHostPromise = null;

export async function getSharedSessionHost() {
  if (!sharedHostPromise) {
    sharedHostPromise = createSharedHost();
  }
  return sharedHostPromise;
}

async function createSharedHost() {
  const bridgeUrl = (import.meta.env?.VITE_SESSION_BRIDGE_URL || DEFAULT_BRIDGE_URL).replace(/\/$/, '');
  const host = new SharedSessionHost(bridgeUrl);
  await host.initialize();
  return host;
}

class SharedSessionHost {
  constructor(bridgeUrl) {
    this.bridgeUrl = bridgeUrl;
    this.listeners = new Set();
    this.logs = [];
    this.state = {};
    this.memory = new Uint8Array(65536);
    this.previousMemory = new Uint8Array(65536);
    this.disassembly = [];
    this.tele7State = null;
    this.pollTimer = null;
    this.connected = false;
    this.unavailableReason = '';
  }

  async initialize() {
    this._emitLog(`Connecting to shared host bridge at ${this.bridgeUrl}...`);
    try {
      await this._refreshSnapshot();
    } catch (error) {
      await this._markUnavailable(error);
    }
    this._startPolling();
  }

  subscribe(listener) {
    this.listeners.add(listener);
    listener(this.snapshot());
    return () => {
      this.listeners.delete(listener);
    };
  }

  snapshot() {
    return {
      state: this.state,
      memory: this.memory,
      previousMemory: this.previousMemory,
      disassembly: this.disassembly,
      tele7State: this.tele7State,
      logs: this.logs,
      connected: this.connected,
      unavailableReason: this.unavailableReason,
      bridgeUrl: this.bridgeUrl,
    };
  }

  _startPolling() {
    if (this.pollTimer) {
      return;
    }

    this.pollTimer = setInterval(() => {
      this._refreshSnapshot().catch((error) => {
        void this._markUnavailable(error);
      });
    }, POLL_INTERVAL_MS);
  }

  dispose() {
    if (this.pollTimer) {
      clearInterval(this.pollTimer);
      this.pollTimer = null;
    }
  }

  async _refreshSnapshot() {
    const response = await this._get(`/v1/snapshot?before=${SNAPSHOT_BEFORE}&after=${SNAPSHOT_AFTER}`);
    const payload = response.snapshot || {};

    this.previousMemory = new Uint8Array(this.memory);
    this.state = payload.state || {};
    this.memory = Uint8Array.from(payload.memory || []);
    this.disassembly = (payload.disassembly || []).map((row) => ({
      addr: row.addr_start,
      len: row.len_bytes,
      op: row.mnemonic,
      args: row.operands,
      is_illegal: row.is_illegal,
      raw: row.raw_words,
    }));
    this.tele7State = payload.tele7
      ? {
          enabled: payload.tele7.enabled,
          blink_phase: payload.tele7.blink_phase,
          border_color: payload.tele7.border_color,
          buffer: payload.tele7.buffer,
        }
      : null;

    if (!this.connected) {
      this._emitLog('Attach session detected.');
    }
    this.connected = true;
    this.unavailableReason = '';
    this._emit();
  }

  async _markUnavailable(error) {
    const reason = await this._resolveUnavailableReason(error);
    const statusChanged = this.connected || this.unavailableReason !== reason;

    this.connected = false;
    this.unavailableReason = reason;

    if (statusChanged) {
      this._emitLog(reason);
    } else {
      this._emit();
    }
  }

  async _resolveUnavailableReason(error) {
    const message = String(error?.message ?? error);
    const bridgeHealthy = await this._isBridgeHealthy();

    if (!bridgeHealthy) {
      return `No attach bridge at ${this.bridgeUrl}. Start a VS Code debug session first.`;
    }

    if (message.toLowerCase().includes('program not launched')) {
      return 'Bridge is reachable, but there is no active debug session yet.';
    }

    return `Bridge is reachable, but snapshot polling failed: ${message}`;
  }

  async _isBridgeHealthy() {
    try {
      await this._get('/v1/health');
      return true;
    } catch {
      return false;
    }
  }

  async _get(path) {
    const response = await fetch(`${this.bridgeUrl}${path}`);
    return this._jsonResponse(response);
  }

  async _jsonResponse(response) {
    let json = {};
    try {
      json = await response.json();
    } catch {
      json = {};
    }

    if (!response.ok) {
      const message = json.error || `${response.status} ${response.statusText}`;
      throw new Error(message);
    }

    return json;
  }

  _emitLog(msg) {
    this.logs = [...this.logs, { ts: Date.now(), msg }];
    if (this.logs.length > 500) {
      this.logs = this.logs.slice(this.logs.length - 500);
    }
    this._emit();
  }

  _emit() {
    const payload = this.snapshot();
    this.listeners.forEach((listener) => listener(payload));
  }
}
