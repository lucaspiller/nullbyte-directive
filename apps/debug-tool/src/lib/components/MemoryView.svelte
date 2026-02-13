<script>
  export let memory;
  export let startAddress = 0x0000;
  export let rows = 16;
  export let cols = 16;

  $: displayRows = Array.from({ length: rows }, (_, r) => {
    const addr = startAddress + r * cols;
    const bytes = Array.from({ length: cols }, (_, c) => {
      const idx = addr + c;
      return memory?.[idx] || 0;
    });
    return { addr, bytes };
  });

  function getChar(byte) {
    if (byte >= 32 && byte <= 126) return String.fromCharCode(byte);
    return '.';
  }
</script>

<div class="p-4 border border-panel-border bg-panel-bg font-mono text-xs overflow-auto">
  <h2 class="text-accent-primary mb-2 font-bold border-b border-panel-border pb-1">MEMORY (0x{startAddress.toString(16).padStart(4, '0')})</h2>
  <div class="table w-full border-collapse">
    {#each displayRows as row}
      <div class="table-row hover:bg-white/5">
        <div class="table-cell pr-4 text-accent-warning">
          0x{row.addr.toString(16).padStart(4, '0').toUpperCase()}
        </div>
        {#each row.bytes as byte}
          <div class="table-cell px-1 text-center w-6">
            <span class={byte === 0 ? "opacity-30" : "text-white"}>
              {byte.toString(16).padStart(2, '0').toUpperCase()}
            </span>
          </div>
        {/each}
        <div class="table-cell pl-4 text-terminal-fg opacity-60 whitespace-pre">
          | {row.bytes.map(getChar).join('')} |
        </div>
      </div>
    {/each}
  </div>
</div>
