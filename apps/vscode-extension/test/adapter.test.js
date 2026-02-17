const assert = require("node:assert/strict");
const path = require("node:path");
const test = require("node:test");
const { spawn } = require("node:child_process");

const ADAPTER_PATH = path.join(__dirname, "..", "src", "dap", "adapter.js");

class DapClient {
  constructor(proc) {
    this.proc = proc;
    this.buffer = Buffer.alloc(0);
    this.seq = 1;
    this.messages = [];
    this.waiters = [];

    proc.stdout.on("data", (chunk) => this.onData(chunk));
  }

  onData(chunk) {
    this.buffer = Buffer.concat([this.buffer, chunk]);
    while (true) {
      const headerEnd = this.buffer.indexOf("\r\n\r\n");
      if (headerEnd === -1) {
        return;
      }

      const header = this.buffer.slice(0, headerEnd).toString("utf8");
      const match = header.match(/Content-Length: (\d+)/i);
      if (!match) {
        this.buffer = Buffer.alloc(0);
        return;
      }

      const length = Number.parseInt(match[1], 10);
      const bodyStart = headerEnd + 4;
      const total = bodyStart + length;
      if (this.buffer.length < total) {
        return;
      }

      const message = JSON.parse(this.buffer.slice(bodyStart, total).toString("utf8"));
      this.buffer = this.buffer.slice(total);

      this.messages.push(message);
      this.flushWaiters();
    }
  }

  flushWaiters() {
    const remaining = [];
    for (const waiter of this.waiters) {
      const index = this.messages.findIndex(waiter.predicate);
      if (index >= 0) {
        const [msg] = this.messages.splice(index, 1);
        waiter.resolve(msg);
      } else {
        remaining.push(waiter);
      }
    }
    this.waiters = remaining;
  }

  async request(command, args = {}) {
    const reqSeq = this.seq++;
    const body = JSON.stringify({
      seq: reqSeq,
      type: "request",
      command,
      arguments: args,
    });

    this.proc.stdin.write(`Content-Length: ${Buffer.byteLength(body)}\r\n\r\n${body}`);

    return this.waitFor(
      (msg) => msg.type === "response" && msg.request_seq === reqSeq,
      3000,
    );
  }

  async waitFor(predicate, timeoutMs) {
    const existing = this.messages.findIndex(predicate);
    if (existing >= 0) {
      const [msg] = this.messages.splice(existing, 1);
      return msg;
    }

    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.waiters = this.waiters.filter((w) => w.resolve !== resolve);
        reject(new Error("timed out waiting for DAP message"));
      }, timeoutMs);

      this.waiters.push({
        predicate,
        resolve: (msg) => {
          clearTimeout(timer);
          resolve(msg);
        },
      });
    });
  }
}

function spawnAdapter() {
  const proc = spawn(process.execPath, [ADAPTER_PATH], {
    stdio: ["pipe", "pipe", "pipe"],
  });
  return { proc, client: new DapClient(proc) };
}

test("bind breakpoints and continue produces stopped event", async (t) => {
  const { proc, client } = spawnAdapter();
  t.after(() => proc.kill());

  const init = await client.request("initialize", {});
  assert.equal(init.success, true);

  const launch = await client.request("launch", { program: "programs/blinker.n1.md" });
  assert.equal(launch.success, true);
  await client.waitFor(
    (msg) => msg.type === "event" && msg.event === "stopped" && msg.body.reason === "entry",
    3000,
  );

  const setBreakpoints = await client.request("setBreakpoints", {
    source: { path: "programs/blinker.n1.md" },
    breakpoints: [{ line: 10 }, { line: 12 }],
  });

  assert.equal(setBreakpoints.success, true);
  assert.equal(setBreakpoints.body.breakpoints.length, 2);
  assert.equal(setBreakpoints.body.breakpoints[0].verified, true);

  const cont = await client.request("continue", { threadId: 1 });
  assert.equal(cont.success, true);

  const stopped = await client.waitFor(
    (msg) => msg.type === "event" && msg.event === "stopped",
    3000,
  );
  assert.equal(stopped.body.reason, "breakpoint");
});

test("step flow exposes stack, scopes, and variables", async (t) => {
  const { proc, client } = spawnAdapter();
  t.after(() => proc.kill());

  assert.equal((await client.request("initialize", {})).success, true);
  assert.equal(
    (await client.request("launch", { program: "programs/tele7_self_test.n1.md" })).success,
    true,
  );
  await client.waitFor(
    (msg) => msg.type === "event" && msg.event === "stopped" && msg.body.reason === "entry",
    3000,
  );

  const next = await client.request("next", { threadId: 1 });
  assert.equal(next.success, true);

  const stopped = await client.waitFor(
    (msg) => msg.type === "event" && msg.event === "stopped",
    3000,
  );
  assert.equal(stopped.body.reason, "step");

  const stack = await client.request("stackTrace", { threadId: 1 });
  assert.equal(stack.success, true);
  assert.equal(stack.body.stackFrames.length, 1);

  const scopes = await client.request("scopes", {
    frameId: stack.body.stackFrames[0].id,
  });
  assert.equal(scopes.success, true);
  assert.equal(scopes.body.scopes.length, 2);

  const registerScope = scopes.body.scopes.find((scope) => scope.name === "Registers");
  const memoryScope = scopes.body.scopes.find((scope) => scope.name === "Memory");

  const regs = await client.request("variables", {
    variablesReference: registerScope.variablesReference,
  });
  assert.equal(regs.success, true);
  assert.ok(regs.body.variables.some((v) => v.name === "PC"));

  const memory = await client.request("variables", {
    variablesReference: memoryScope.variablesReference,
  });
  assert.equal(memory.success, true);
  assert.ok(memory.body.variables.length > 0);
});
