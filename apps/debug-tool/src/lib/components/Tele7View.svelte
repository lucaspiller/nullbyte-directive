<script>
  let { tele7State = null } = $props();

  const COLS = 40;
  const ROWS = 25;

  const COLOR_PALETTE = [
    '#000000', // 0 - Black
    '#FF0000', // 1 - Red
    '#00FF00', // 2 - Green
    '#FFFF00', // 3 - Yellow
    '#0000FF', // 4 - Blue
    '#FF00FF', // 5 - Magenta
    '#00FFFF', // 6 - Cyan
    '#FFFFFF', // 7 - White
  ];

  function getChar(byte) {
    if (byte >= 32 && byte <= 126) return String.fromCharCode(byte);
    if (byte === 0) return '\u00A0';
    return '.';
  }

  function getFgColor(fg) {
    return COLOR_PALETTE[fg] || COLOR_PALETTE[7];
  }

  function getBgColor(bg) {
    return COLOR_PALETTE[bg] || COLOR_PALETTE[0];
  }

  let rows = $derived.by(() => {
    if (!tele7State || !tele7State.buffer) {
      return Array(ROWS).fill(null).map(() => Array(COLS).fill({ char: ' ', fg: 7, bg: 0 }));
    }

    const buffer = tele7State.buffer;
    const result = [];
    let currentFg = 7;
    let currentBg = 0;
    let mosaicMode = false;
    let flashMode = false;

    for (let row = 0; row < ROWS; row++) {
      const rowChars = [];
      for (let col = 0; col < COLS; col++) {
        const byteIdx = row * COLS + col;
        const wordIdx = Math.floor(byteIdx / 2);
        const word = buffer[wordIdx] || [0, 0];
        const byte = byteIdx % 2 === 0 ? word[0] : word[1];

        // Handle control codes
        if (byte < 0x20) {
          if (byte >= 0x00 && byte <= 0x07) {
            currentFg = byte;
          } else if (byte >= 0x10 && byte <= 0x17) {
            currentBg = byte - 0x10;
          } else if (byte === 0x18) {
            mosaicMode = true;
          } else if (byte === 0x19) {
            mosaicMode = false;
          } else if (byte === 0x1A) {
            flashMode = true;
          } else if (byte === 0x1B) {
            flashMode = false;
          }
          rowChars.push({ char: ' ', fg: currentFg, bg: currentBg, control: true });
        } else {
          let char = getChar(byte);
          // Handle blink - suppress foreground during blink off phase
          if (flashMode && tele7State.blink_phase) {
            rowChars.push({ char, fg: currentBg, bg: currentBg, control: false });
          } else {
            rowChars.push({ char, fg: currentFg, bg: currentBg, control: false });
          }
        }
      }
      result.push(rowChars);
      // Reset line state at start of each row
      currentFg = 7;
      currentBg = 0;
      mosaicMode = false;
      flashMode = false;
    }
    return result;
  });
</script>

<div class="flex flex-col h-full bg-terminal-bg border border-panel-border">
  <div class="bg-panel-bg border-b border-panel-border px-2 py-1 flex justify-between items-center">
    <span class="text-xs font-bold text-accent-primary">TELE-7</span>
    <span class="text-xs text-terminal-fg opacity-50">
      {#if tele7State?.enabled}
        ON
      {:else}
        OFF
      {/if}
      {#if tele7State?.blink_phase}
        <span class="text-accent-warning">*</span>
      {/if}
    </span>
  </div>
  
  <div class="flex-1 overflow-auto p-1 font-mono text-xs leading-tight" 
       style="background-color: {COLOR_PALETTE[tele7State?.border_color || 0] || '#000000'}">
    {#each rows as row, r}
      <div class="flex">
        {#each row as cell, c}
          <span 
            class="inline-block w-3 text-center"
            style="color: {getFgColor(cell.fg)}; background-color: {getBgColor(cell.bg)};"
          >{cell.char}</span>
        {/each}
      </div>
    {/each}
  </div>
</div>
