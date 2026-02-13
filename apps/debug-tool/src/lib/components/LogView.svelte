<script>
  export let logs = [];
</script>

<div class="h-full flex flex-col p-4 border border-panel-border bg-panel-bg font-mono text-xs overflow-hidden">
  <h2 class="text-accent-primary mb-2 font-bold border-b border-panel-border pb-1">LOG</h2>
  <div class="flex-1 overflow-y-auto space-y-1">
    {#each logs as log}
      <div class="text-terminal-fg opacity-80 border-l-2 pl-2 border-accent-warning">
        <span class="text-white/40 text-[0.8em] block">{new Date(log.ts).toLocaleTimeString()}</span>
        {log.msg}
      </div>
    {/each}
  </div>
</div>
