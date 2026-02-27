<script lang="ts">
  import { onMount } from 'svelte';

  import initWasm, { WasmCore } from './wasm/emulator_wasm.js';
  import RegisterView from '$lib/components/RegisterView.svelte';
  import MemoryView from '$lib/components/MemoryView.svelte';
  import DisassemblyView from '$lib/components/DisassemblyView.svelte';
  import LogView from '$lib/components/LogView.svelte';
  import Tele7View from '$lib/components/Tele7View.svelte';
  import { getSharedSessionHost } from '$lib/session/sharedSessionHost.js';

  const ATTACH_ROUTE = '/attach';
  const DISASM_BEFORE = 3;
  const DISASM_AFTER = 4;
  const TURBO_INTERVAL_MS = 10;
  const SPEEDS = [
    { name: 'SLOW', interval: 100, useTick: false },
    { name: 'FAST', interval: 10, useTick: false },
    { name: 'TURBO', interval: 0, useTick: true },
  ];

  type Mode = 'standalone' | 'attach';

  let mode = $state(resolveMode(globalThis.location?.pathname ?? '/'));
  let state = $state({});
  let memory = $state.raw(new Uint8Array(65536));
  let previousMemory = $state.raw(new Uint8Array(65536));
  let logs = $state([]);
  let isRunning = $state(false);
  let tele7State = $state(null);
  let disassembly = $state([]);
  let speedIndex = $state(1);
  let speedName = $derived(SPEEDS[speedIndex].name);
  let standaloneReady = $state(false);
  let attachConnected = $state(false);
  let attachUnavailableReason = $state('');
  let bridgeUrl = $state('');
  let host = $state(null);
  let core = $state<WasmCore | null>(null);
  let unsubscribeHost: (() => void) | null = null;
  let runTimer: ReturnType<typeof setInterval> | null = null;

  function resolveMode(pathname: string): Mode {
    const normalized = pathname.replace(/\/+$/, '') || '/';
    return normalized === ATTACH_ROUTE ? 'attach' : 'standalone';
  }

  function appendLog(msg: string) {
    logs = [...logs, { ts: Date.now(), msg }];
    if (logs.length > 500) {
      logs = logs.slice(logs.length - 500);
    }
  }

  function toDisassemblyRows(rows: any[]) {
    return rows.map((row) => ({
      addr: row.addr_start ?? row.addr ?? 0,
      len: row.len_bytes ?? row.len ?? 2,
      op: row.mnemonic ?? row.op ?? '.word',
      args: row.operands ?? row.args ?? '',
      is_illegal: Boolean(row.is_illegal),
      raw: row.raw_words ?? row.raw ?? [],
    }));
  }

  function normalizeTele7(raw: any) {
    if (!raw) {
      return null;
    }

    return {
      enabled: Boolean(raw.enabled),
      blink_phase: Boolean(raw.blink_phase ?? raw.blinkPhase),
      border_color: raw.border_color ?? raw.borderColor ?? 0,
      buffer: raw.buffer ?? raw.display_buffer ?? [],
    };
  }

  function syncStandaloneSnapshot(trackDiff = true) {
    if (!core) {
      return;
    }

    const nextMemory = new Uint8Array(core.get_memory());
    previousMemory = trackDiff ? new Uint8Array(memory) : new Uint8Array(nextMemory.length);
    memory = nextMemory;
    state = core.get_state() || {};

    const pc = state?.arch?.pc || 0;
    disassembly = toDisassemblyRows(core.disassemble_window(pc, DISASM_BEFORE, DISASM_AFTER) || []);

    try {
      tele7State = normalizeTele7(core.get_tele7_state());
    } catch {
      tele7State = null;
    }
  }

  function stopRunLoop() {
    if (runTimer) {
      clearInterval(runTimer);
      runTimer = null;
    }
    isRunning = false;
  }

  async function initializeStandalone() {
    try {
      await initWasm();
      core = new WasmCore();
      standaloneReady = true;
      appendLog('Standalone mode ready (local WasmCore).');
      syncStandaloneSnapshot(false);
    } catch (error) {
      appendLog(`WASM init error: ${String(error)}`);
    }
  }

  async function initializeAttach() {
    host = await getSharedSessionHost();
    unsubscribeHost = host.subscribe((snapshot) => {
      state = snapshot.state || {};
      memory = snapshot.memory ? Uint8Array.from(snapshot.memory) : new Uint8Array(65536);
      previousMemory = snapshot.previousMemory
        ? Uint8Array.from(snapshot.previousMemory)
        : new Uint8Array(65536);
      disassembly = Array.isArray(snapshot.disassembly)
        ? snapshot.disassembly
        : toDisassemblyRows(snapshot.disassembly || []);
      tele7State = normalizeTele7(snapshot.tele7State);
      logs = snapshot.logs || [];
      attachConnected = Boolean(snapshot.connected);
      attachUnavailableReason = snapshot.unavailableReason || '';
      bridgeUrl = snapshot.bridgeUrl || '';
      isRunning = false;
    });
  }

  async function stepStandalone() {
    if (mode !== 'standalone' || !core) {
      return;
    }

    try {
      core.step();
      syncStandaloneSnapshot();
    } catch (error) {
      stopRunLoop();
      appendLog(`Step Error: ${String(error)}`);
    }
  }

  async function tickStandalone() {
    if (mode !== 'standalone' || !core) {
      return;
    }

    try {
      core.tick();
      syncStandaloneSnapshot();
    } catch (error) {
      stopRunLoop();
      appendLog(`Tick Error: ${String(error)}`);
    }
  }

  function runStandalone() {
    if (mode !== 'standalone' || !core || isRunning) {
      return;
    }

    isRunning = true;
    const speed = SPEEDS[speedIndex];
    if (speed.useTick) {
      runTimer = setInterval(() => {
        void tickStandalone();
      }, TURBO_INTERVAL_MS);
    } else {
      runTimer = setInterval(() => {
        void stepStandalone();
      }, speed.interval);
    }
  }

  function toggleRun() {
    if (isRunning) {
      pauseStandalone();
    } else {
      runStandalone();
    }
  }

  function pauseStandalone() {
    stopRunLoop();
    if (mode === 'standalone') {
      syncStandaloneSnapshot();
    }
  }

  function resetStandalone() {
    if (mode !== 'standalone' || !core) {
      return;
    }

    stopRunLoop();
    core.reset_and_reload();
    syncStandaloneSnapshot(false);
    appendLog('Core reset.');
  }

  function cycleSpeed() {
    if (mode !== 'standalone') {
      return;
    }

    const wasRunning = isRunning;
    if (wasRunning) {
      stopRunLoop();
    }

    speedIndex = (speedIndex + 1) % SPEEDS.length;
    if (wasRunning) {
      runStandalone();
    }
  }

  async function handleFileSelect(event: Event) {
    if (mode !== 'standalone' || !core) {
      return;
    }

    const target = event.currentTarget as HTMLInputElement;
    const file = target?.files?.[0];
    if (!file) {
      return;
    }

    const lowerName = file.name.toLowerCase();
    if (!(lowerName.endsWith('.n1') || lowerName.endsWith('.n1.md'))) {
      appendLog('Load Error: only .n1 and .n1.md files are supported.');
      target.value = '';
      return;
    }

    try {
      const source = await file.text();
      stopRunLoop();
      core.assemble_and_load_program(source, file.name);
      syncStandaloneSnapshot(false);
      appendLog(`Assembled and loaded ${file.name}`);
    } catch (error) {
      appendLog(`Load Error: ${String(error)}`);
    } finally {
      target.value = '';
    }
  }

  function formatRunState(runState: any) {
    if (!runState) return 'UNKNOWN';
    if (typeof runState === 'string') return runState;
    const key = Object.keys(runState)[0];
    const value = runState[key];
    if (value !== null && value !== undefined) {
      return `${key}(${value})`;
    }
    return key;
  }

  function formatFault() {
    if (state?.latched_fault !== undefined && state?.latched_fault !== null) {
      return state.latched_fault;
    }

    const runState = state?.run_state;
    if (runState && typeof runState === 'object' && runState.FaultLatched !== undefined) {
      return runState.FaultLatched;
    }

    return 'NONE';
  }

  function modeBadge() {
    return mode === 'attach' ? 'ATTACH (READ ONLY)' : 'STANDALONE';
  }

  onMount(() => {
    if (mode === 'attach') {
      void initializeAttach();
    } else {
      void initializeStandalone();
    }

    return () => {
      stopRunLoop();
      if (unsubscribeHost) {
        unsubscribeHost();
      }
      if (host?.dispose) {
        host.dispose();
      }
      if (core) {
        core.free();
      }
    };
  });
</script>

{#if mode === 'standalone' && !standaloneReady}
  <div class="h-screen w-screen flex items-center justify-center bg-terminal-bg text-accent-primary font-mono text-xl animate-pulse">
    > BOOTING STANDALONE CORE...
  </div>
{:else if mode === 'attach' && !host}
  <div class="h-screen w-screen flex items-center justify-center bg-terminal-bg text-accent-primary font-mono text-xl animate-pulse">
    > CONNECTING TO ATTACH BRIDGE...
  </div>
{:else}
  <div class="h-screen w-screen bg-terminal-bg text-terminal-fg flex flex-col overflow-hidden font-mono">
    <header class="bg-panel-bg border-b border-panel-border p-2 flex items-center justify-between shadow-md z-10">
      <div class="flex items-center space-x-4">
        <h1 class="text-xl font-bold text-accent-primary mr-4 tracking-tight">NULLBYTE::DEBUG</h1>
        <span class="text-xs uppercase tracking-wide border border-panel-border px-2 py-1">{modeBadge()}</span>
        <a
          class="px-3 py-1 border border-accent-primary hover:bg-accent-primary hover:text-black rounded transition-colors text-xs uppercase tracking-wide"
          href={mode === 'attach' ? '/' : '/attach'}
        >
          {mode === 'attach' ? 'OPEN STANDALONE' : 'OPEN ATTACH'}
        </a>

        {#if mode === 'standalone'}
          <div class="flex space-x-2">
            <button
              class="px-3 py-1 bg-accent-primary text-black font-bold hover:bg-white hover:text-black rounded transition-colors text-xs uppercase tracking-wide"
              onclick={toggleRun}
            >
              {isRunning ? 'PAUSE' : 'RUN'}
            </button>
            <button
              class="px-3 py-1 border border-terminal-fg hover:bg-white hover:text-black rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed text-xs uppercase tracking-wide"
              onclick={stepStandalone}
              disabled={isRunning}
            >
              STEP
            </button>
            <button
              class="px-3 py-1 border border-terminal-fg hover:bg-white hover:text-black rounded transition-colors text-xs uppercase tracking-wide"
              onclick={resetStandalone}
            >
              RESET
            </button>
            <button
              class="px-3 py-1 border border-terminal-fg hover:bg-white hover:text-black rounded transition-colors text-xs uppercase tracking-wide {speedIndex === 2 ? 'bg-accent-warning text-black font-bold' : ''}"
              onclick={cycleSpeed}
            >
              {speedName}
            </button>
          </div>

          <div class="border-l border-panel-border pl-4 ml-2 flex items-center">
            <input
              type="file"
              onchange={handleFileSelect}
              class="text-xs text-terminal-fg
                file:mr-4 file:py-1 file:px-2
                file:rounded-full file:border-0
                file:text-xs file:font-semibold
                file:bg-panel-border file:text-white
                hover:file:bg-white hover:file:text-black
                cursor-pointer"
            />
          </div>
        {/if}
      </div>

      <div class="text-right text-xs font-mono opacity-50 pr-4">
        PC: <span class="text-white">0x{(state?.arch?.pc || 0).toString(16).padStart(4, '0').toUpperCase()}</span> |
        TICK: <span class="text-white">{state?.arch?.tick || 0}</span>
      </div>
    </header>

    {#if mode === 'attach' && !attachConnected}
      <main class="flex-1 flex items-center justify-center bg-black/20 p-6">
        <div class="max-w-2xl w-full bg-panel-bg border border-panel-border p-6 space-y-3">
          <h2 class="text-accent-warning font-bold text-lg">NO ACTIVE ATTACH SESSION</h2>
          <p class="text-sm opacity-80">{attachUnavailableReason || 'Bridge is unreachable.'}</p>
          <p class="text-xs opacity-60">
            Bridge URL: <span class="text-white">{bridgeUrl || 'http://127.0.0.1:48765'}</span>
          </p>
          <p class="text-xs opacity-60">
            Start debugging in VS Code (Launch Nullbyte Directive), then open or refresh <span class="text-white">/attach</span>.
          </p>
        </div>
      </main>
    {:else}
      <main class="flex-1 grid grid-cols-12 gap-1 p-1 overflow-hidden bg-black/20">
        <div class="col-span-3 flex flex-col gap-1 h-full min-h-0">
          <div class="flex-1 overflow-hidden bg-panel-bg border border-panel-border flex flex-col">
            <DisassemblyView pc={state?.arch?.pc || 0} rows={disassembly} />
          </div>
          <div class="h-1/3 overflow-hidden bg-panel-bg border border-panel-border flex flex-col">
            <RegisterView arch={state?.arch || {}} />
          </div>
        </div>

        <div class="col-span-6 flex flex-col gap-1 bg-panel-bg border border-panel-border h-full overflow-hidden p-1">
          <div class="flex-1 overflow-hidden">
            <MemoryView memory={memory} previousMemory={previousMemory} pc={state?.arch?.pc || 0} />
          </div>
          <div class="h-48 overflow-hidden">
            <Tele7View tele7State={tele7State} />
          </div>
        </div>

        <div class="col-span-3 flex flex-col bg-panel-bg border border-panel-border h-full overflow-hidden">
          <LogView logs={logs} />
        </div>
      </main>
    {/if}

    <footer class="bg-panel-bg border-t border-panel-border p-1 text-xs flex justify-between px-4 opacity-60">
      <span>STATUS: {formatRunState(state?.run_state)} | MODE: {modeBadge()}</span>
      <span>FAULT: {formatFault()}</span>
    </footer>
  </div>
{/if}
