const path = require("node:path");

class InMemorySessionHostClient {
  constructor() {
    this.reset();
  }

  reset() {
    this.sessionId = `${Date.now().toString(16)}-${Math.random().toString(16).slice(2, 8)}`;
    this.registers = {
      R0: 0,
      R1: 0,
      R2: 0,
      R3: 0,
      R4: 0,
      R5: 0,
      R6: 0,
      R7: 0,
      PC: 0,
      SP: 0xfffe,
      FLAGS: 0,
      TICK: 0,
      CAP: 0,
      CAUSE: 0,
      EVP: 0,
    };
    this.breakpoints = new Map();
    this.program = null;
    this.memory = new Uint8Array(0x10000);
    this.paused = true;
  }

  async launch(programPath) {
    this.reset();
    this.program = path.resolve(programPath);
    return {
      debugSessionId: this.sessionId,
      buildId: "local-build",
      layoutId: "local-layout",
      diagnostics: [],
    };
  }

  async setBreakpoints(filePath, lines) {
    const key = path.resolve(filePath);
    this.breakpoints.set(key, [...new Set(lines)].sort((a, b) => a - b));
    return lines.map((line) => ({
      line,
      verified: true,
      source: key,
    }));
  }

  async continueExecution() {
    this.paused = false;
    this.registers.PC = (this.registers.PC + 2) & 0xffff;
    this.registers.TICK = (this.registers.TICK + 1) & 0xffff;
    this.paused = true;
    return {
      stopReason: "breakpoint",
      fault: null,
    };
  }

  async instructionStep() {
    this.registers.PC = (this.registers.PC + 2) & 0xffff;
    this.registers.TICK = (this.registers.TICK + 1) & 0xffff;
    return {
      stopReason: "step",
      fault: null,
    };
  }

  async pause() {
    this.paused = true;
  }

  async readState() {
    return {
      paused: this.paused,
      registers: { ...this.registers },
      runState: "Running",
    };
  }

  async readMemoryWindow(start, len) {
    return {
      start,
      bytes: Array.from(this.memory.slice(start, start + len)),
    };
  }

  async disconnect() {
    this.paused = true;
  }
}

function createDefaultHostClient() {
  return new InMemorySessionHostClient();
}

module.exports = {
  InMemorySessionHostClient,
  createDefaultHostClient,
};
