<script>
  export let pc;
  export let memory;

  // Placeholder logic for disassembly
  // In a real implementation, this would call into the emulator's decoder.
  $: disassembledInstructions = [
    { addr: pc,     op: 'NOP',      args: '' },
    { addr: pc + 1, op: '???',      args: '0x00' },
  ];
</script>

<div class="p-4 border border-panel-border bg-panel-bg font-mono text-sm">
  <h2 class="text-accent-primary mb-2 font-bold border-b border-panel-border pb-1">DISASSEMBLY</h2>
  <div class="text-terminal-fg">
    {#each disassembledInstructions as instr}
      <div class="flex space-x-4 {instr.addr === pc ? 'bg-accent-primary text-black font-bold' : ''}">
        <span class="w-16">0x{instr.addr.toString(16).padStart(4, '0').toUpperCase()}</span>
        <span class="flex-1">{instr.op} {instr.args}</span>
      </div>
    {/each}
  </div>
</div>
