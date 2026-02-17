import initWasm, { WasmCore } from '../../wasm/emulator_wasm.js';

const TICK_INTERVAL_MS = 10;
const SPEEDS = [
  { name: 'SLOW', interval: 100 },
  { name: 'FAST', interval: 10 },
  { name: 'TURBO', interval: 0 },
];

let sharedHostPromise = null;

export async function getSharedSessionHost() {
  if (!sharedHostPromise) {
    sharedHostPromise = createSharedHost();
  }
  return sharedHostPromise;
}

async function createSharedHost() {
  await initWasm();
  const core = new WasmCore();
  return new SharedSessionHost(core);
}

class SharedSessionHost {
  constructor(core) {
    this.core = core;
    this.listeners = new Set();
    this.logs = [];
    this.state = {};
    this.memory = new Uint8Array(65536);
    this.previousMemory = new Uint8Array(65536);
    this.tele7State = null;
    this.isRunning = false;
    this.speedIndex = 1;
    this.controlOwner = null;
    this.interval = null;
    this._emitLog('Shared session initialized.');
    this._updateState();
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
      tele7State: this.tele7State,
      logs: this.logs,
      isRunning: this.isRunning,
      speedIndex: this.speedIndex,
      speedName: SPEEDS[this.speedIndex].name,
      controlOwner: this.controlOwner,
    };
  }

  requestControl(clientId) {
    if (this.controlOwner && this.controlOwner !== clientId) {
      return false;
    }
    this.controlOwner = clientId;
    this._emitLog(`Control attached (${clientId.slice(0, 8)}).`);
    this._emit();
    return true;
  }

  releaseControl(clientId) {
    if (this.controlOwner !== clientId) {
      return;
    }
    this.pause(clientId, true);
    this.controlOwner = null;
    this._emitLog(`Control released (${clientId.slice(0, 8)}).`);
    this._emit();
  }

  step(clientId) {
    this._requireControl(clientId);
    this.core.step();
    this._updateState();
  }

  tick(clientId) {
    this._requireControl(clientId);
    const outcome = this.core.tick();
    this._updateState();
    if (outcome?.final_step?.Fault !== undefined) {
      this.pause(clientId, true);
      this._emitLog(`Fault: ${outcome.final_step.Fault}`);
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
      this.interval = setInterval(() => {
        if (this.isRunning) {
          this.tick(clientId);
        }
      }, TICK_INTERVAL_MS);
      requestAnimationFrame(this._turboUpdateState.bind(this));
    } else {
      this.interval = setInterval(() => {
        if (this.isRunning) {
          this.step(clientId);
        }
      }, currentSpeed.interval);
    }
    this._emit();
  }

  pause(clientId, force = false) {
    if (!force) {
      this._requireControl(clientId);
    }
    if (!this.isRunning) {
      return;
    }
    this.isRunning = false;
    clearInterval(this.interval);
    this.interval = null;
    this._emit();
  }

  reset(clientId) {
    this._requireControl(clientId);
    this.pause(clientId, true);
    this.core.reset_and_reload();
    this._updateState();
    this._emitLog('Core reset.');
  }

  setSpeed(clientId, speedIndex) {
    this._requireControl(clientId);
    const wasRunning = this.isRunning;
    if (wasRunning) {
      this.pause(clientId, true);
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
    this.pause(clientId, true);
    this.core.reset();

    const fileName = file.name.toLowerCase();
    if (fileName.endsWith('.n1') || fileName.endsWith('.n1.md')) {
      const source = await file.text();
      this.core.assemble_and_load_program(source, file.name);
      this._updateState();
      this._emitLog(`Assembled and loaded source from ${file.name}`);
      return;
    }

    const bytes = new Uint8Array(await file.arrayBuffer());
    this.core.load_program(bytes);
    this._updateState();
    this._emitLog(`Loaded ${bytes.length} bytes from ${file.name}`);
  }

  _requireControl(clientId) {
    if (this.controlOwner !== clientId) {
      throw new Error('Session controls are owned by another client.');
    }
  }

  _turboUpdateState() {
    if (!this.isRunning) {
      return;
    }
    this._updateState();
    requestAnimationFrame(this._turboUpdateState.bind(this));
  }

  _updateState() {
    this.previousMemory = new Uint8Array(this.memory);
    this.state = this.core.get_state();
    this.memory = this.core.get_memory();
    this.tele7State = this.core.get_tele7_state();
    this._emit();
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
