const path = require("node:path");
const fs = require("node:fs");
const { spawn } = require("node:child_process");
const readline = require("node:readline");

class StdioSessionHostClient {
  constructor() {
    this.child = null;
    this.readline = null;
    this.nextId = 1;
    this.pending = new Map();
    this.workspaceRoot = null;
  }

  async launch(programPath, cwdHint) {
    const root = findWorkspaceRoot(cwdHint || process.cwd());
    const resolvedProgram = path.isAbsolute(programPath)
      ? programPath
      : path.resolve(root, programPath);
    await this.ensureStarted(root);
    const launch = await this.request("launch", { program: resolvedProgram });
    return {
      debugSessionId: launch.ids.debug_session_id,
      buildId: launch.ids.build_id,
      layoutId: launch.ids.layout_id,
      diagnostics: diagnosticsFromWarnings(launch.warnings || []),
    };
  }

  async reload(programPath, cwdHint, mode = "hot_patch") {
    const root = findWorkspaceRoot(cwdHint || this.workspaceRoot || process.cwd());
    const resolvedProgram = path.isAbsolute(programPath)
      ? programPath
      : path.resolve(root, programPath);
    await this.ensureStarted(root);
    const reload = await this.request("reload", {
      program: resolvedProgram,
      mode,
    });
    return {
      debugSessionId: reload.ids.debug_session_id,
      buildId: reload.ids.build_id,
      layoutId: reload.ids.layout_id,
      diagnostics: diagnosticsFromWarnings(reload.warnings || []),
      appliedMode: reload.applied_mode,
      cleanRunReason: reload.clean_run_reason || null,
    };
  }

  async setBreakpoints(filePath, lines) {
    await this.ensureStarted();
    const resolved = path.isAbsolute(filePath)
      ? filePath
      : path.resolve(this.workspaceRoot || process.cwd(), filePath);
    const result = await this.request("set_breakpoints", {
      file: resolved,
      lines,
    });
    return result.breakpoints.map((bp) => ({
      line: bp.line,
      verified: bp.verified,
      source: resolved,
      message: bp.message,
    }));
  }

  async continueExecution() {
    await this.ensureStarted();
    return this.request("continue_execution", {});
  }

  async instructionStep() {
    await this.ensureStarted();
    return this.request("instruction_step", {});
  }

  async pause() {
    await this.ensureStarted();
    await this.request("pause", {});
  }

  async readState() {
    await this.ensureStarted();
    return this.request("read_state", {});
  }

  async readMemoryWindow(start, len) {
    await this.ensureStarted();
    return this.request("read_memory_window", { start, len });
  }

  async disconnect() {
    if (!this.child) {
      return;
    }

    try {
      await this.request("disconnect", {});
    } catch {
      // Best effort during shutdown.
    }

    this.child.kill();
    this.child = null;
    if (this.readline) {
      this.readline.close();
      this.readline = null;
    }
  }

  async ensureStarted(rootHint) {
    if (this.child) {
      return;
    }

    const workspaceRoot = rootHint || this.workspaceRoot || findWorkspaceRoot(process.cwd());
    this.workspaceRoot = workspaceRoot;

    this.child = spawn(
      "cargo",
      [
        "run",
        "--quiet",
        "-p",
        "emulator-debug-session",
        "--bin",
        "debug-session-stdio-host",
      ],
      {
        cwd: workspaceRoot,
        stdio: ["pipe", "pipe", "pipe"],
      },
    );

    this.child.stderr.on("data", (chunk) => {
      const text = chunk.toString("utf8").trim();
      if (text.length > 0) {
        process.stderr.write(`${text}\n`);
      }
    });

    this.child.on("exit", (code, signal) => {
      const pending = Array.from(this.pending.values());
      this.pending.clear();
      this.child = null;
      for (const waiter of pending) {
        waiter.reject(
          new Error(`session host exited before response (code=${code}, signal=${signal})`),
        );
      }
    });

    this.readline = readline.createInterface({ input: this.child.stdout });
    this.readline.on("line", (line) => {
      let msg;
      try {
        msg = JSON.parse(line);
      } catch {
        return;
      }

      const pending = this.pending.get(msg.id);
      if (!pending) {
        return;
      }

      this.pending.delete(msg.id);
      if (msg.ok) {
        pending.resolve(msg.result);
      } else {
        pending.reject(new Error(msg.error || "session host request failed"));
      }
    });
  }

  request(method, params) {
    if (!this.child || !this.child.stdin.writable) {
      return Promise.reject(new Error("session host is not running"));
    }

    const id = this.nextId++;
    const payload = JSON.stringify({ id, method, params });
    this.child.stdin.write(`${payload}\n`);

    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
    });
  }

  async tickStep() {
    await this.ensureStarted();
    return this.request("tick_step", {});
  }

  async readVisualizerSnapshot(before, after) {
    await this.ensureStarted();
    return this.request("read_visualizer_snapshot", { before, after });
  }
}

function diagnosticsFromWarnings(warnings) {
  return warnings.map((message) => ({
    severity: "warning",
    message,
  }));
}

function findWorkspaceRoot(startDir) {
  let current = path.resolve(startDir);
  while (true) {
    const candidate = path.join(current, "Cargo.toml");
    try {
      const content = fs.readFileSync(candidate, "utf8");
      if (content.includes("crates/emulator-debug-session")) {
        return current;
      }
    } catch {
      // Continue walking upward.
    }

    const parent = path.dirname(current);
    if (parent === current) {
      break;
    }
    current = parent;
  }

  throw new Error("unable to locate workspace root for emulator-debug-session");
}

function createDefaultHostClient() {
  return new StdioSessionHostClient();
}

module.exports = {
  StdioSessionHostClient,
  createDefaultHostClient,
};
