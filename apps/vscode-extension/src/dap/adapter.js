#!/usr/bin/env node

const { createDefaultHostClient } = require("./session_host_client");

const THREAD_ID = 1;
const STACK_FRAME_ID = 100;
const REG_SCOPE_ID = 200;
const MEM_SCOPE_ID = 201;

class DapAdapter {
  constructor(hostClient) {
    this.host = hostClient;
    this.seq = 1;
    this.buffer = Buffer.alloc(0);
  }

  start() {
    process.stdin.on("data", (chunk) => this.onData(chunk));
  }

  onData(chunk) {
    this.buffer = Buffer.concat([this.buffer, chunk]);
    while (true) {
      const headerEnd = this.buffer.indexOf("\r\n\r\n");
      if (headerEnd === -1) {
        return;
      }

      const headerText = this.buffer.slice(0, headerEnd).toString("utf8");
      const lengthMatch = headerText.match(/Content-Length: (\d+)/i);
      if (!lengthMatch) {
        this.buffer = Buffer.alloc(0);
        return;
      }

      const contentLength = Number.parseInt(lengthMatch[1], 10);
      const bodyStart = headerEnd + 4;
      const totalLength = bodyStart + contentLength;
      if (this.buffer.length < totalLength) {
        return;
      }

      const body = this.buffer.slice(bodyStart, totalLength).toString("utf8");
      this.buffer = this.buffer.slice(totalLength);

      let message;
      try {
        message = JSON.parse(body);
      } catch {
        continue;
      }

      if (message.type === "request") {
        this.handleRequest(message).catch((error) => {
          this.sendErrorResponse(message, String(error?.message || error));
        });
      }
    }
  }

  async handleRequest(request) {
    switch (request.command) {
      case "initialize":
        this.sendResponse(request, {
          supportsConfigurationDoneRequest: true,
        });
        this.sendEvent("initialized", {});
        return;
      case "configurationDone":
        this.sendResponse(request, {});
        return;
      case "launch": {
        const program = request.arguments?.program;
        if (!program) {
          this.sendErrorResponse(request, "launch requires 'program'");
          return;
        }

        const result = await this.host.launch(program);
        this.sendResponse(request, {});
        this.sendEvent("output", {
          category: "console",
          output: `session=${result.debugSessionId} build=${result.buildId} layout=${result.layoutId}\n`,
        });
        this.sendEvent("stopped", {
          reason: "entry",
          threadId: THREAD_ID,
          allThreadsStopped: true,
        });
        return;
      }
      case "threads":
        this.sendResponse(request, {
          threads: [{ id: THREAD_ID, name: "Nullbyte Core" }],
        });
        return;
      case "setBreakpoints": {
        const sourcePath = request.arguments?.source?.path;
        const sourceBreakpoints = request.arguments?.breakpoints || [];
        const lines = sourceBreakpoints.map((bp) => bp.line).filter(Number.isInteger);
        if (!sourcePath) {
          this.sendErrorResponse(request, "setBreakpoints requires source.path");
          return;
        }

        const resolved = await this.host.setBreakpoints(sourcePath, lines);
        this.sendResponse(request, {
          breakpoints: resolved.map((bp) => ({
            verified: bp.verified,
            line: bp.line,
          })),
        });
        return;
      }
      case "continue": {
        const outcome = await this.host.continueExecution();
        this.sendResponse(request, { allThreadsContinued: true });
        this.sendStopFromOutcome(outcome, "breakpoint");
        return;
      }
      case "next": {
        const outcome = await this.host.instructionStep();
        this.sendResponse(request, {});
        this.sendStopFromOutcome(outcome, "step");
        return;
      }
      case "pause":
        await this.host.pause();
        this.sendResponse(request, {});
        this.sendEvent("stopped", {
          reason: "pause",
          threadId: THREAD_ID,
          allThreadsStopped: true,
        });
        return;
      case "stackTrace": {
        const state = await this.host.readState();
        this.sendResponse(request, {
          stackFrames: [
            {
              id: STACK_FRAME_ID,
              name: `PC=0x${toHex(state.registers.PC, 4)}`,
              line: 1,
              column: 1,
            },
          ],
          totalFrames: 1,
        });
        return;
      }
      case "scopes":
        this.sendResponse(request, {
          scopes: [
            {
              name: "Registers",
              variablesReference: REG_SCOPE_ID,
              expensive: false,
            },
            {
              name: "Memory",
              variablesReference: MEM_SCOPE_ID,
              expensive: false,
            },
          ],
        });
        return;
      case "variables": {
        const variablesReference = request.arguments?.variablesReference;
        if (variablesReference === REG_SCOPE_ID) {
          const state = await this.host.readState();
          const variables = Object.entries(state.registers).map(([name, value]) => ({
            name,
            value: `0x${toHex(value, 4)}`,
            variablesReference: 0,
          }));
          this.sendResponse(request, { variables });
          return;
        }

        if (variablesReference === MEM_SCOPE_ID) {
          const window = await this.host.readMemoryWindow(0, 32);
          const variables = [];
          for (let i = 0; i < window.bytes.length; i += 16) {
            const row = window.bytes.slice(i, i + 16);
            variables.push({
              name: `0x${toHex(window.start + i, 4)}`,
              value: row.map((b) => toHex(b, 2)).join(" "),
              variablesReference: 0,
            });
          }
          this.sendResponse(request, { variables });
          return;
        }

        this.sendResponse(request, { variables: [] });
        return;
      }
      case "disconnect":
        await this.host.disconnect();
        this.sendResponse(request, {});
        this.sendEvent("terminated", {});
        return;
      default:
        this.sendResponse(request, {});
    }
  }

  sendStopFromOutcome(outcome, fallbackReason) {
    if (outcome?.fault) {
      this.sendEvent("output", {
        category: "stderr",
        output: `fault: ${outcome.fault}\n`,
      });
    }

    this.sendEvent("stopped", {
      reason: outcome?.stopReason || fallbackReason,
      threadId: THREAD_ID,
      allThreadsStopped: true,
    });
  }

  sendEvent(event, body) {
    this.writeMessage({
      seq: this.seq++,
      type: "event",
      event,
      body,
    });
  }

  sendResponse(request, body) {
    this.writeMessage({
      seq: this.seq++,
      type: "response",
      request_seq: request.seq,
      command: request.command,
      success: true,
      body,
    });
  }

  sendErrorResponse(request, message) {
    this.writeMessage({
      seq: this.seq++,
      type: "response",
      request_seq: request.seq,
      command: request.command,
      success: false,
      message,
    });
  }

  writeMessage(message) {
    const body = Buffer.from(JSON.stringify(message), "utf8");
    process.stdout.write(`Content-Length: ${body.length}\r\n\r\n`);
    process.stdout.write(body);
  }
}

function toHex(value, width) {
  return Number(value).toString(16).padStart(width, "0");
}

const adapter = new DapAdapter(createDefaultHostClient());
adapter.start();
