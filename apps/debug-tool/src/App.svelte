<script lang="ts">
  import { onMount } from 'svelte';

  import RegisterView from '$lib/components/RegisterView.svelte';
  import MemoryView from '$lib/components/MemoryView.svelte';
  import DisassemblyView from '$lib/components/DisassemblyView.svelte';
  import LogView from '$lib/components/LogView.svelte';
  import initWasm, { WasmCore } from './wasm/emulator_wasm.js';

  let state = $state({});
  let memory = $state.raw(new Uint8Array(65536));
  let previousMemory = $state.raw(new Uint8Array(65536));
  let logs = $state([]);
  let isRunning = $state(false);
  let interval;
  let wasm = $state({ core: null });
  let lastLoadedProgram = null;

  const TICK_INTERVAL_MS = 10;

  const SPEEDS = [
    { name: 'SLOW', interval: 100 },
    { name: 'FAST', interval: 10 },
    { name: 'TURBO', interval: 0 },
  ];
  let speedIndex = $state(1);

  async function loadWasm() {
    try {
      await initWasm();
      wasm.core = new WasmCore();
      logs = [...logs, { ts: Date.now(), msg: `Core initialized.`}]
    } catch (e) {
      console.error('Error loading WASM', e);
      logs = [...logs, { ts: Date.now(), msg: `Error: ${e.message}` }];
    }
  }

  onMount(() => {
    loadWasm()
  });

  function updateState() {
    if (!wasm.core) return;
    try {
      previousMemory = new Uint8Array(memory);
      state = wasm.core.get_state();
      memory = wasm.core.get_memory();
    } catch (e) {
        console.error("State update failed:", e);
    }
  }

  function step() {
    if (!wasm.core) return;
    try {
      wasm.core.step();
      updateState();
    } catch (e) {
      console.error(e);
      logs = [...logs, { ts: Date.now(), msg: `Step Error: ${e.message}` }];
    }
  }

  function tick() {
    if (!wasm.core) return null;
    try {
      return wasm.core.tick();
    } catch (e) {
      console.error(e);
      logs = [...logs, { ts: Date.now(), msg: `Tick Error: ${e.message}` }];
      return null;
    }
  }

  function run() {
    if (isRunning) return;
    isRunning = true;
    const currentSpeed = SPEEDS[speedIndex];
    if (currentSpeed.interval === 0) {
      startTurbo();
    } else {
      interval = setInterval(() => {
        if (wasm.core) {
          step();
        }
      }, currentSpeed.interval);
    }
  }

  function startTurbo() {
    interval = setInterval(() => {
      if (wasm.core && isRunning) {
        const outcome = tick();
        if (outcome?.final_step?.Fault !== undefined) {
          pause();
          updateState();
          logs = [...logs, { ts: Date.now(), msg: `Fault: ${outcome.final_step.Fault}` }];
        }
      }
    }, TICK_INTERVAL_MS);

    requestAnimationFrame(function turboUpdateState() {
      if (!isRunning) return;
      updateState();
      requestAnimationFrame(turboUpdateState);
    });
  }

  function pause() {
    if (!isRunning) return;
    isRunning = false;
    clearInterval(interval);
  }

  function reset() {
    if (!wasm.core) return;
    pause();
    wasm.core.reset();
    if (lastLoadedProgram) {
      wasm.core.load_program(lastLoadedProgram);
    }
    updateState();
    logs = [...logs, { ts: Date.now(), msg: "Core reset." }];
  }

  function cycleSpeed() {
    const wasRunning = isRunning;
    if (wasRunning) pause();
    speedIndex = (speedIndex + 1) % SPEEDS.length;
    if (wasRunning) run();
  }

  let fileInput;
  function handleFileSelect(e) {
    const file = e.target.files[0];
    if (!file) return;

    const reader = new FileReader();
    reader.onload = (evt) => {
      const arrayBuffer = evt.target.result;
      const bytes = new Uint8Array(arrayBuffer);
      lastLoadedProgram = bytes;
      wasm.core.load_program(bytes);
      updateState();
      logs = [...logs, { ts: Date.now(), msg: `Loaded ${bytes.length} bytes from ${file.name}` }];
    };
    reader.readAsArrayBuffer(file);
  }

  function formatRunState(runState) {
    if (!runState) return 'UNKNOWN';
    if (typeof runState === 'string') return runState;
    const key = Object.keys(runState)[0];
    const value = runState[key];
    if (value !== null && value !== undefined) {
      return `${key}(${value})`;
    }
    return key;
  }
</script>

{#if !wasm.core}
  <div class="h-screen w-screen flex items-center justify-center bg-terminal-bg text-accent-primary font-mono text-xl animate-pulse">
    > INITIALIZING CORE SYSTEM...
  </div>
{:else}
<div class="h-screen w-screen bg-terminal-bg text-terminal-fg flex flex-col overflow-hidden font-mono">
  <!-- Toolbar -->
  <header class="bg-panel-bg border-b border-panel-border p-2 flex items-center justify-between shadow-md z-10">
    <div class="flex items-center space-x-4">
      <h1 class="text-xl font-bold text-accent-primary mr-4 tracking-tight">NULLBYTE::DEBUG</h1>
      
      <div class="flex space-x-2">
        <button 
          class="px-3 py-1 bg-accent-primary text-black font-bold hover:bg-white hover:text-black rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed text-xs uppercase tracking-wide"
          onclick={isRunning ? pause : run}
        >
          {isRunning ? 'PAUSE' : 'RUN'}
        </button>
        <button 
          class="px-3 py-1 border border-terminal-fg hover:bg-white hover:text-black rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed text-xs uppercase tracking-wide"
          onclick={step}
          disabled={isRunning}
        >
          STEP
        </button>
        <button 
          class="px-3 py-1 border border-terminal-fg hover:bg-white hover:text-black rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed text-xs uppercase tracking-wide"
          onclick={reset}
        >
          RESET
        </button>
        <button 
          class="px-3 py-1 border border-terminal-fg hover:bg-white hover:text-black rounded transition-colors text-xs uppercase tracking-wide {speedIndex === 2 ? 'bg-accent-warning text-black font-bold' : ''}"
          onclick={cycleSpeed}
        >
          {SPEEDS[speedIndex].name}
        </button>
      </div>

      <div class="border-l border-panel-border pl-4 ml-4 flex items-center">
        <input 
          type="file" 
          bind:this={fileInput} 
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
    </div>

    <div class="text-right text-xs font-mono opacity-50 pr-4">
      PC: <span class="text-white">0x{(state?.arch?.pc || 0).toString(16).padStart(4, '0').toUpperCase()}</span> | 
      TICK: <span class="text-white">{(state?.arch?.tick || 0)}</span>
    </div>
  </header>

  <!-- Main Grid -->
  <main class="flex-1 grid grid-cols-12 gap-1 p-1 overflow-hidden bg-black/20">
    <!-- Left Column: Disassembly & Registers -->
    <div class="col-span-3 flex flex-col gap-1 h-full min-h-0">
      <div class="flex-1 overflow-hidden bg-panel-bg border border-panel-border flex flex-col">
        <DisassemblyView pc={state?.arch?.pc || 0} memory={memory} wasmCore={wasm.core} />
      </div>
      <div class="h-1/3 overflow-hidden bg-panel-bg border border-panel-border flex flex-col">
        <RegisterView arch={state?.arch || {}} />
      </div>
    </div>

    <!-- Center Column: Memory -->
    <div class="col-span-6 flex flex-col bg-panel-bg border border-panel-border h-full overflow-hidden">
      <MemoryView memory={memory} previousMemory={previousMemory} pc={state?.arch?.pc || 0} />
    </div>

    <!-- Right Column: Logs / Events -->
    <div class="col-span-3 flex flex-col bg-panel-bg border border-panel-border h-full overflow-hidden">
      <LogView logs={logs} />
    </div>
  </main>
  
  <!-- Status Bar -->
  <footer class="bg-panel-bg border-t border-panel-border p-1 text-xs flex justify-between px-4 opacity-60">
    <span>STATUS: {formatRunState(state?.run_state)}</span>
    <span>FAULT: {state?.latched_fault || 'NONE'}</span>
  </footer>
</div>
{/if}
