<script lang="ts">
  import { onMount } from 'svelte';

  import RegisterView from '$lib/components/RegisterView.svelte';
  import MemoryView from '$lib/components/MemoryView.svelte';
  import DisassemblyView from '$lib/components/DisassemblyView.svelte';
  import LogView from '$lib/components/LogView.svelte';
  import Tele7View from '$lib/components/Tele7View.svelte';
  import { getSharedSessionHost } from '$lib/session/sharedSessionHost.js';

  let state = $state({});
  let memory = $state.raw(new Uint8Array(65536));
  let previousMemory = $state.raw(new Uint8Array(65536));
  let logs = $state([]);
  let isRunning = $state(false);
  let host = $state(null);
  let tele7State = $state(null);
  let disassembly = $state([]);
  let speedName = $state('FAST');
  let speedIndex = $state(1);
  let controlOwner = $state(null);
  let secondaryControlsEnabled = $state(false);
  let clientId = $state('');
  let unsubscribeHost = null;

  async function attachToSession() {
    try {
      clientId = globalThis.crypto?.randomUUID?.() ?? `client-${Date.now()}`;
      host = await getSharedSessionHost();
      unsubscribeHost = host.subscribe((snapshot) => {
        state = snapshot.state;
        memory = snapshot.memory;
        previousMemory = snapshot.previousMemory;
        disassembly = snapshot.disassembly;
        tele7State = snapshot.tele7State;
        logs = snapshot.logs;
        isRunning = snapshot.isRunning;
        speedName = snapshot.speedName;
        speedIndex = snapshot.speedIndex;
        controlOwner = snapshot.controlOwner;
      });
    } catch (e) {
      logs = [...logs, { ts: Date.now(), msg: `Error: ${e.message}` }];
    }
  }

  onMount(() => {
    attachToSession();
    return () => {
      if (secondaryControlsEnabled && host) {
        host.releaseControl(clientId).catch(() => {});
      }
      if (unsubscribeHost) {
        unsubscribeHost();
      }
    };
  });

  function hasControl() {
    return secondaryControlsEnabled && controlOwner === clientId;
  }

  function step() {
    if (!host || !hasControl()) return;
    host.step(clientId).catch((e) => {
      logs = [...logs, { ts: Date.now(), msg: `Step Error: ${e.message}` }];
    });
  }

  function run() {
    if (!host || !hasControl()) return;
    try {
      host.run(clientId);
    } catch (e) {
      logs = [...logs, { ts: Date.now(), msg: `Run Error: ${e.message}` }];
    }
  }

  function pause() {
    if (!host || !hasControl()) return;
    host.pause(clientId).catch((e) => {
      logs = [...logs, { ts: Date.now(), msg: `Pause Error: ${e.message}` }];
    });
  }

  function reset() {
    if (!host || !hasControl()) return;
    host.reset(clientId).catch((e) => {
      logs = [...logs, { ts: Date.now(), msg: `Reset Error: ${e.message}` }];
    });
  }

  function cycleSpeed() {
    if (!host || !hasControl()) return;
    try {
      host.setSpeed(clientId, (speedIndex + 1) % 3);
    } catch (e) {
      logs = [...logs, { ts: Date.now(), msg: `Speed Error: ${e.message}` }];
    }
  }

  let fileInput;
  async function handleFileSelect(e) {
    const file = e.target.files[0];
    if (!file) return;
    if (!host || !hasControl()) return;
    try {
      await host.loadProgramFromFile(clientId, file);
    } catch (err) {
      logs = [...logs, { ts: Date.now(), msg: `Load Error: ${err.message ?? err}` }];
    }
  }

  function toggleSecondaryControls() {
    if (!host) return;
    if (!secondaryControlsEnabled) {
      host.requestControl(clientId).then((granted) => {
        if (!granted) {
          logs = [...logs, { ts: Date.now(), msg: 'Control lock unavailable: another client owns controls.' }];
          return;
        }
        secondaryControlsEnabled = true;
      }).catch((e) => {
        logs = [...logs, { ts: Date.now(), msg: `Control Error: ${e.message}` }];
      });
      return;
    }

    secondaryControlsEnabled = false;
    host.releaseControl(clientId).catch((e) => {
      logs = [...logs, { ts: Date.now(), msg: `Release Error: ${e.message}` }];
    });
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

{#if !host}
  <div class="h-screen w-screen flex items-center justify-center bg-terminal-bg text-accent-primary font-mono text-xl animate-pulse">
    > ATTACHING TO SHARED SESSION...
  </div>
{:else}
<div class="h-screen w-screen bg-terminal-bg text-terminal-fg flex flex-col overflow-hidden font-mono">
  <!-- Toolbar -->
  <header class="bg-panel-bg border-b border-panel-border p-2 flex items-center justify-between shadow-md z-10">
    <div class="flex items-center space-x-4">
      <h1 class="text-xl font-bold text-accent-primary mr-4 tracking-tight">NULLBYTE::DEBUG</h1>
      
      <div class="flex space-x-2">
        <button
          class="px-3 py-1 border border-accent-primary hover:bg-accent-primary hover:text-black rounded transition-colors text-xs uppercase tracking-wide"
          onclick={toggleSecondaryControls}
        >
          {secondaryControlsEnabled ? 'DETACH CTRL' : 'ATTACH CTRL'}
        </button>
        <button 
          class="px-3 py-1 bg-accent-primary text-black font-bold hover:bg-white hover:text-black rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed text-xs uppercase tracking-wide"
          onclick={isRunning ? pause : run}
          disabled={!hasControl()}
        >
          {isRunning ? 'PAUSE' : 'RUN'}
        </button>
        <button 
          class="px-3 py-1 border border-terminal-fg hover:bg-white hover:text-black rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed text-xs uppercase tracking-wide"
          onclick={step}
          disabled={isRunning || !hasControl()}
        >
          STEP
        </button>
        <button 
          class="px-3 py-1 border border-terminal-fg hover:bg-white hover:text-black rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed text-xs uppercase tracking-wide"
          onclick={reset}
          disabled={!hasControl()}
        >
          RESET
        </button>
        <button 
          class="px-3 py-1 border border-terminal-fg hover:bg-white hover:text-black rounded transition-colors text-xs uppercase tracking-wide disabled:opacity-50 disabled:cursor-not-allowed {speedIndex === 2 ? 'bg-accent-warning text-black font-bold' : ''}"
          onclick={cycleSpeed}
          disabled={!hasControl()}
        >
          {speedName}
        </button>
      </div>

      <div class="border-l border-panel-border pl-4 ml-4 flex items-center">
        <input 
          type="file" 
          bind:this={fileInput} 
          onchange={handleFileSelect} 
          disabled={!hasControl()}
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
        <DisassemblyView pc={state?.arch?.pc || 0} rows={disassembly} />
      </div>
      <div class="h-1/3 overflow-hidden bg-panel-bg border border-panel-border flex flex-col">
        <RegisterView arch={state?.arch || {}} />
      </div>
    </div>

    <!-- Center Column: Memory -->
    <div class="col-span-6 flex flex-col gap-1 bg-panel-bg border border-panel-border h-full overflow-hidden p-1">
      <div class="flex-1 overflow-hidden">
        <MemoryView memory={memory} previousMemory={previousMemory} pc={state?.arch?.pc || 0} />
      </div>
      <div class="h-48 overflow-hidden">
        <Tele7View tele7State={tele7State} />
      </div>
    </div>

    <!-- Right Column: Logs / Events -->
    <div class="col-span-3 flex flex-col bg-panel-bg border border-panel-border h-full overflow-hidden">
      <LogView logs={logs} />
    </div>
  </main>
  
  <!-- Status Bar -->
  <footer class="bg-panel-bg border-t border-panel-border p-1 text-xs flex justify-between px-4 opacity-60">
    <span>STATUS: {formatRunState(state?.run_state)} | MODE: ATTACH | CONTROL: {hasControl() ? 'ENABLED' : (controlOwner ? 'REMOTE' : 'NONE')}</span>
    <span>FAULT: {state?.latched_fault || 'NONE'}</span>
  </footer>
</div>
{/if}
