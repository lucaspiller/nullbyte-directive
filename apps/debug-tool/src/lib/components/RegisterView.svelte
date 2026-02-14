<script>
  export let arch = {};

  const registers = ['r0', 'r1', 'r2', 'r3', 'r4', 'r5', 'r6', 'r7'];
</script>

<div class="p-4 border border-panel-border bg-panel-bg font-mono text-sm">
  <h2 class="text-accent-primary mb-2 font-bold border-b border-panel-border pb-1">REGISTERS</h2>
  <div class="grid grid-cols-2 gap-x-4 gap-y-1">
    {#each registers as reg, i}
      <div class="flex justify-between">
        <span class="text-terminal-fg opacity-70">{reg.toUpperCase()}</span>
        <span class="text-accent-warning">
          0x{(arch?.gpr?.[i] || 0).toString(16).padStart(4, '0').toUpperCase()}
        </span>
      </div>
    {/each}
    <div class="flex justify-between mt-2 pt-2 border-t border-panel-border col-span-2">
        <span class="text-terminal-fg opacity-70">PC</span>
        <span class="text-white">0x{(arch?.pc || 0).toString(16).padStart(4, '0').toUpperCase()}</span>
    </div>
    <div class="flex justify-between col-span-2">
        <span class="text-terminal-fg opacity-70">SP</span>
        <span class="text-white">0x{(arch?.sp || 0).toString(16).padStart(4, '0').toUpperCase()}</span>
    </div>
    <div class="flex justify-between col-span-2">
        <span class="text-terminal-fg opacity-70">FLAGS</span>
        <span class="text-white">0x{(arch?.flags || 0).toString(16).padStart(4, '0').toUpperCase()}</span>
    </div>
  </div>
</div>
