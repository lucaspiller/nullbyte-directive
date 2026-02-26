const http = require("node:http");
const fs = require("node:fs/promises");
const os = require("node:os");
const path = require("node:path");

const DEFAULT_PORT = 48765;

class SessionBridgeServer {
  constructor(host) {
    this.host = host;
    this.server = null;
    this.port = Number.parseInt(process.env.NULLBYTE_SESSION_BRIDGE_PORT || "", 10) || DEFAULT_PORT;
    this.controlOwner = null;
    this.lastLaunch = null;
    this.tempRoot = null;
  }

  async start() {
    if (this.server) {
      return;
    }

    this.server = http.createServer((req, res) => {
      this.handleRequest(req, res).catch((error) => {
        this.writeJson(res, 500, { error: String(error?.message || error) });
      });
    });

    await new Promise((resolve, reject) => {
      this.server.once("error", reject);
      this.server.listen(this.port, "127.0.0.1", () => {
        this.server.off("error", reject);
        resolve();
      });
    });
  }

  async close() {
    if (!this.server) {
      return;
    }

    await new Promise((resolve) => this.server.close(resolve));
    this.server = null;
  }

  setLastLaunch(program, cwd) {
    this.lastLaunch = { program, cwd };
  }

  async handleRequest(req, res) {
    this.writeCors(res);
    if (req.method === "OPTIONS") {
      res.statusCode = 204;
      res.end();
      return;
    }

    const url = new URL(req.url || "/", `http://${req.headers.host || "127.0.0.1"}`);
    if (req.method === "GET" && url.pathname === "/v1/health") {
      this.writeJson(res, 200, {
        ok: true,
        port: this.port,
        controlOwner: this.controlOwner,
      });
      return;
    }

    if (req.method === "GET" && url.pathname === "/v1/control") {
      this.writeJson(res, 200, {
        controlOwner: this.controlOwner,
      });
      return;
    }

    if (req.method === "POST" && url.pathname === "/v1/control/request") {
      const body = await readBody(req);
      const clientId = String(body.clientId || "");
      if (!clientId) {
        this.writeJson(res, 400, { error: "clientId is required" });
        return;
      }

      if (!this.controlOwner || this.controlOwner === clientId) {
        this.controlOwner = clientId;
        this.writeJson(res, 200, { granted: true, controlOwner: this.controlOwner });
        return;
      }

      this.writeJson(res, 200, { granted: false, controlOwner: this.controlOwner });
      return;
    }

    if (req.method === "POST" && url.pathname === "/v1/control/release") {
      const body = await readBody(req);
      const clientId = String(body.clientId || "");
      if (clientId && this.controlOwner === clientId) {
        this.controlOwner = null;
      }
      this.writeJson(res, 200, { controlOwner: this.controlOwner });
      return;
    }

    if (req.method === "GET" && url.pathname === "/v1/snapshot") {
      const before = parseNatural(url.searchParams.get("before"), 3);
      const after = parseNatural(url.searchParams.get("after"), 4);
      const snapshot = await this.host.readVisualizerSnapshot(before, after);
      this.writeJson(res, 200, {
        controlOwner: this.controlOwner,
        snapshot,
      });
      return;
    }

    if (req.method === "POST" && url.pathname === "/v1/step") {
      const body = await readBody(req);
      if (!this.assertControl(res, body.clientId)) {
        return;
      }
      const outcome = await this.host.instructionStep();
      this.writeJson(res, 200, { outcome });
      return;
    }

    if (req.method === "POST" && url.pathname === "/v1/tick") {
      const body = await readBody(req);
      if (!this.assertControl(res, body.clientId)) {
        return;
      }
      const outcome = await this.host.tickStep();
      this.writeJson(res, 200, { outcome });
      return;
    }

    if (req.method === "POST" && url.pathname === "/v1/pause") {
      const body = await readBody(req);
      if (!this.assertControl(res, body.clientId)) {
        return;
      }
      await this.host.pause();
      this.writeJson(res, 200, { ok: true });
      return;
    }

    if (req.method === "POST" && url.pathname === "/v1/reset") {
      const body = await readBody(req);
      if (!this.assertControl(res, body.clientId)) {
        return;
      }
      if (!this.lastLaunch) {
        this.writeJson(res, 400, { error: "no launch context available for reset" });
        return;
      }
      const report = await this.host.launch(this.lastLaunch.program, this.lastLaunch.cwd);
      this.writeJson(res, 200, { report });
      return;
    }

    if (req.method === "POST" && url.pathname === "/v1/load_source") {
      const body = await readBody(req);
      if (!this.assertControl(res, body.clientId)) {
        return;
      }

      const source = String(body.source || "");
      const fileName = String(body.fileName || "program.n1.md");
      if (!source.trim()) {
        this.writeJson(res, 400, { error: "source is required" });
        return;
      }

      const sourcePath = await this.writeTempSource(fileName, source);
      const report = await this.host.launch(sourcePath, process.cwd());
      this.lastLaunch = { program: sourcePath, cwd: process.cwd() };
      this.writeJson(res, 200, { report, sourcePath });
      return;
    }

    this.writeJson(res, 404, { error: "not found" });
  }

  assertControl(res, clientId) {
    const id = String(clientId || "");
    if (!id) {
      this.writeJson(res, 400, { error: "clientId is required" });
      return false;
    }
    if (!this.controlOwner || this.controlOwner !== id) {
      this.writeJson(res, 409, {
        error: "controls are owned by another client",
        controlOwner: this.controlOwner,
      });
      return false;
    }
    return true;
  }

  writeCors(res) {
    res.setHeader("Access-Control-Allow-Origin", "*");
    res.setHeader("Access-Control-Allow-Methods", "GET,POST,OPTIONS");
    res.setHeader("Access-Control-Allow-Headers", "content-type");
  }

  writeJson(res, status, body) {
    res.statusCode = status;
    res.setHeader("content-type", "application/json; charset=utf-8");
    res.end(JSON.stringify(body));
  }

  async writeTempSource(fileName, source) {
    if (!this.tempRoot) {
      this.tempRoot = await fs.mkdtemp(path.join(os.tmpdir(), "nullbyte-session-"));
    }
    const safeName = path.basename(fileName).replace(/[^a-zA-Z0-9._-]/g, "_");
    const target = path.join(this.tempRoot, safeName || "program.n1.md");
    await fs.writeFile(target, source, "utf8");
    return target;
  }
}

function parseNatural(value, fallback) {
  const parsed = Number.parseInt(String(value || ""), 10);
  if (!Number.isFinite(parsed) || parsed < 0) {
    return fallback;
  }
  return parsed;
}

async function readBody(req) {
  const chunks = [];
  for await (const chunk of req) {
    chunks.push(chunk);
  }
  if (chunks.length === 0) {
    return {};
  }

  const text = Buffer.concat(chunks).toString("utf8").trim();
  if (!text) {
    return {};
  }

  return JSON.parse(text);
}

module.exports = {
  SessionBridgeServer,
  DEFAULT_PORT,
};
