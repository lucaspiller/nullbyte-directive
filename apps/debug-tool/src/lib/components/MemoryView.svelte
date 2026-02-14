<script>
  let { memory, previousMemory, pc, cols = 16 } = $props();

  const PAGE_ROWS = 32;
  const PAGE_SIZE = PAGE_ROWS * cols;

  const REGIONS = [
    { name: 'ROM', start: 0x0000, end: 0x3FFF, color: 'text-green-400' },
    { name: 'RAM', start: 0x4000, end: 0xDFFF, color: 'text-blue-400' },
    { name: 'MMIO', start: 0xE000, end: 0xEFFF, color: 'text-yellow-400' },
    { name: 'DIAG', start: 0xF000, end: 0xF0FF, color: 'text-purple-400' },
    { name: 'RSVD', start: 0xF100, end: 0xFFFF, color: 'text-red-400' },
  ];

  let viewStart = $state(0x0000);

  function jumpToRegion(region) {
    viewStart = region.start;
  }

  function pageUp() {
    viewStart = Math.max(0, viewStart - PAGE_SIZE);
  }

  function pageDown() {
    viewStart = Math.min(0x10000 - PAGE_SIZE, viewStart + PAGE_SIZE);
  }

  function getRegionForAddress(addr) {
    return REGIONS.find(r => addr >= r.start && addr <= r.end);
  }

  function isCurrentRegion(region) {
    return viewStart >= region.start && viewStart <= region.end;
  }

  let displayRows = $derived(Array.from({ length: PAGE_ROWS }, (_, r) => {
    const addr = viewStart + r * cols;
    const bytes = Array.from({ length: cols }, (_, c) => {
      const idx = addr + c;
      return memory?.[idx] || 0;
    });
    return { addr, bytes };
  }));

  let endAddress = $derived(viewStart + PAGE_SIZE - 1);

  function getChar(byte) {
    if (byte >= 32 && byte <= 126) return String.fromCharCode(byte);
    return '.';
  }

  function getByteClass(idx) {
    const isPc = idx === pc || idx === pc + 1;
    const isChanged = previousMemory && memory?.[idx] !== previousMemory?.[idx];
    const region = getRegionForAddress(idx);
    
    if (isPc) return "text-black bg-accent-primary font-bold";
    if (isChanged) return "text-black bg-accent-warning";
    if (memory?.[idx] === 0) return "opacity-30";
    return region?.color || "text-white";
  }
</script>

<div class="h-full flex flex-col border border-panel-border bg-panel-bg font-mono text-xs overflow-hidden">
  <div class="flex items-center justify-between px-4 py-2 border-b border-panel-border">
    <h2 class="text-accent-primary font-bold">MEMORY</h2>
    <div class="flex items-center gap-1">
      {#each REGIONS as region}
        <button
          class="px-2 py-0.5 text-xs border transition-colors {isCurrentRegion(region) ? region.color + ' border-current bg-current/20 font-bold' : region.color + ' border-current/30 hover:bg-current/10'}"
          onclick={() => jumpToRegion(region)}
          title="0x{region.start.toString(16).toUpperCase()}..0x{region.end.toString(16).toUpperCase()}"
        >
          {region.name}
        </button>
      {/each}
    </div>
  </div>
  
  <div class="flex items-center justify-between px-4 py-1 border-b border-panel-border text-xs opacity-60">
    <span>0x{viewStart.toString(16).padStart(4, '0').toUpperCase()} .. 0x{endAddress.toString(16).padStart(4, '0').toUpperCase()}</span>
    <div class="flex gap-2">
      <button class="text-terminal-fg hover:text-accent-primary" onclick={pageUp}>▲</button>
      <button class="text-terminal-fg hover:text-accent-primary" onclick={pageDown}>▼</button>
    </div>
  </div>
  
  <div class="flex-1 overflow-auto p-4">
    <div class="table w-full border-collapse">
      {#each displayRows as row}
        <div class="table-row hover:bg-white/5">
          <div class="table-cell pr-4 text-accent-warning">
            0x{row.addr.toString(16).padStart(4, '0').toUpperCase()}
          </div>
          {#each row.bytes as byte, c}
            <div class="table-cell px-1 text-center w-6">
              <span class={getByteClass(row.addr + c)}>
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
</div>
