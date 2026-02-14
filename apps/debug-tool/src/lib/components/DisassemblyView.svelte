<script>
  export let pc;
  export let memory;
  export let wasmCore;

  const NUM_BEFORE = 3;
  const NUM_AFTER = 5;

  $: disassembledInstructions = (() => {
    if (!wasmCore) return [];
    
    try {
      const rows = wasmCore.disassemble_window(pc, NUM_BEFORE, NUM_AFTER);
      return rows.map(row => ({
        addr: row.addr_start,
        len: row.len_bytes,
        op: row.mnemonic,
        args: row.operands,
        is_illegal: row.is_illegal,
        raw: row.raw_words
      }));
    } catch (e) {
      console.error('Disassembly error:', e);
      return [];
    }
  })();

  function isAtPc(instr) {
    return pc >= instr.addr && pc < instr.addr + instr.len;
  }
</script>

<div class="p-4 border border-panel-border bg-panel-bg font-mono text-sm overflow-y-auto h-full">
  <h2 class="text-accent-primary mb-2 font-bold border-b border-panel-border pb-1">DISASSEMBLY</h2>
  <div class="text-terminal-fg">
    {#if disassembledInstructions.length === 0}
      <div class="text-terminal-fg opacity-50">Loading...</div>
    {:else}
      {#each disassembledInstructions as instr}
        <div class="flex space-x-2 {isAtPc(instr) ? 'bg-accent-primary text-black font-bold' : ''} {instr.is_illegal ? 'text-accent-warning' : ''}">
          <span class="w-16 flex-shrink-0">0x{instr.addr.toString(16).padStart(4, '0').toUpperCase()}</span>
          <span class="w-8 flex-shrink-0 text-xs opacity-50">{instr.len}B</span>
          <span class="flex-1">{instr.op} {instr.args}</span>
        </div>
      {/each}
    {/if}
  </div>
</div>
