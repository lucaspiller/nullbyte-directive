const TICK_INTERVAL_MS = 10;
const SPEEDS = [
  { name: 'SLOW', interval: 100 },
  { name: 'FAST', interval: 10 },
  { name: 'TURBO', interval: 0 },
];

const DEFAULT_BRIDGE_URL = 'http://127.0.0.1:48765';
const SNAPSHOT_BEFORE = 3;
const SNAPSHOT_AFTER = 4;

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
    this.isRunning = false;
    this.speedIndex = 1;
    this.controlOwner = null;
    this.pollTimer = null;
    this.runTimer = null;
  }

  async initialize() {
    this._emitLog(`Connecting to shared host bridge at ${this.bridgeUrl}...`);
    await this._refreshSnapshot();
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
      isRunning: this.isRunning,
      speedIndex: this.speedIndex,
      speedName: SPEEDS[this.speedIndex].name,
      controlOwner: this.controlOwner,
    };
  }

  async requestControl(clientId) {
    const response = await this._post('/v1/control/request', { clientId });
    this.controlOwner = response.controlOwner || null;
    if (response.granted) {
      this._emitLog(`Control attached (${clientId.slice(0, 8)}).`);
      this._emit();
      return true;
    }

    this._emit();
    return false;
  }

  async releaseControl(clientId) {
    await this.pause(clientId, true);
    const response = await this._post('/v1/control/release', { clientId });
    this.controlOwner = response.controlOwner || null;
    this._emitLog(`Control released (${clientId.slice(0, 8)}).`);
    this._emit();
  }

  async step(clientId) {
    this._requireControl(clientId);
    await this._post('/v1/step', { clientId });
    await this._refreshSnapshot();
  }

  async tick(clientId) {
    this._requireControl(clientId);
    const outcome = await this._post('/v1/tick', { clientId });
    await this._refreshSnapshot();
    if (outcome?.outcome?.fault) {
      await this.pause(clientId, true);
      this._emitLog(`Fault: ${outcome.outcome.fault}`);
    }
    return outcome;
  }

  run(clientId) {
    this._requireControl(clientId);
    if (this.isRunning) {
      return;
    }

    this.isRunning = true;
    const currentSpeed = SPEEDS[this.speedIndex];
    if (currentSpeed.interval === 0) {
      this.runTimer = setInterval(() => {
        if (this.isRunning) {
          this.tick(clientId).catch((error) => this._onRunError(error));
        }
      }, TICK_INTERVAL_MS);
    } else {
      this.runTimer = setInterval(() => {
        if (this.isRunning) {
          this.step(clientId).catch((error) => this._onRunError(error));
        }
      }, currentSpeed.interval);
    }
    this._emit();
  }

  async pause(clientId, force = false) {
    if (!force) {
      this._requireControl(clientId);
    }

    if (!this.isRunning) {
      if (force) {
        await this._refreshSnapshot();
      }
      return;
    }

    this.isRunning = false;
    clearInterval(this.runTimer);
    this.runTimer = null;
    if (force || this.controlOwner === clientId) {
      try {
        await this._post('/v1/pause', { clientId });
      } catch {
        // Best effort if host is already paused.
      }
    }
    await this._refreshSnapshot();
  }

  async reset(clientId) {
    this._requireControl(clientId);
    await this.pause(clientId, true);
    await this._post('/v1/reset', { clientId });
    await this._refreshSnapshot();
    this._emitLog('Core reset.');
  }

  setSpeed(clientId, speedIndex) {
    this._requireControl(clientId);
    const wasRunning = this.isRunning;
    if (wasRunning) {
      void this.pause(clientId, true);
    }

    this.speedIndex = speedIndex % SPEEDS.length;
    if (wasRunning) {
      this.run(clientId);
    } else {
      this._emit();
    }
  }

  async loadProgramFromFile(clientId, file) {
    this._requireControl(clientId);
    await this.pause(clientId, true);

    const fileName = file.name.toLowerCase();
    if (!(fileName.endsWith('.n1') || fileName.endsWith('.n1.md'))) {
      throw new Error('Only .n1 and .n1.md source files are supported in attach mode.');
    }

    const source = await file.text();
    await this._post('/v1/load_source', {
      clientId,
      fileName: file.name,
      source,
    });
    await this._refreshSnapshot();
    this._emitLog(`Assembled and loaded source from ${file.name}`);
  }

  _requireControl(clientId) {
    if (this.controlOwner !== clientId) {
      throw new Error('Session controls are owned by another client.');
    }
  }

  _startPolling() {
    if (this.pollTimer) {
      return;
    }

    this.pollTimer = setInterval(() => {
      this._refreshSnapshot().catch((error) => {
        this._emitLog(`Snapshot refresh failed: ${error.message}`);
      });
    }, 250);
  }

  async _refreshSnapshot() {
    const response = await this._get(`/v1/snapshot?before=${SNAPSHOT_BEFORE}&after=${SNAPSHOT_AFTER}`);
    const payload = response.snapshot;
    this.controlOwner = response.controlOwner || null;

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
          display_buffer: payload.tele7.buffer,
        }
      : null;

    this._emit();
  }

  async _get(path) {
    const response = await fetch(`${this.bridgeUrl}${path}`);
    return this._jsonResponse(response);
  }

  async _post(path, body) {
    const response = await fetch(`${this.bridgeUrl}${path}`, {
      method: 'POST',
      headers: {
        'content-type': 'application/json',
      },
      body: JSON.stringify(body),
    });
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

  _onRunError(error) {
    this.isRunning = false;
    clearInterval(this.runTimer);
    this.runTimer = null;
    this._emitLog(`Run Error: ${error.message}`);
    this._emit();
  }

  _emit() {
    const payload = this.snapshot();
    this.listeners.forEach((listener) => listener(payload));
  }
}
