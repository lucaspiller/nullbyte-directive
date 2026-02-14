================================================================================
TELE-7 Textual Display Device -- Technical Reference Manual
================================================================================

The TELE-7 is a memory-mapped textual display providing a 40x25 operator
console. It was designed for the Flight Core environment: 16-bit memory access
only, deterministic tick-driven operation, and a hard ceiling on compute budget.
The display feels physically updated -- partial updates during scan are visible
and expected, giving operators a direct view into memory state without hidden
buffering.

This manual describes the display model, page buffer layout, control codes, and
MMIO interface. A programmer writing to the device or an engineer auditing its
behavior should find everything needed here.

================================================================================

1. # At-a-glance

Geometry: 40 columns x 25 rows (1000 character cells) Page buffer: 500 words, 2
characters per word (high/low byte) Character: 0x20-0x7F printable (ROM font),
0x00-0x1F control Colors: 8 foreground + 8 background (0=black through 7=white)
Modes: Mosaic (block graphics), Flash (tick-driven blink) MMIO base:
0xE120-0xE12F (16-bit registers only) Tick rate: 100 Hz, blink divisor defaults
to 50 (2 Hz blink)

================================================================================ 2)
Display Geometry
================================================================================

The TELE-7 presents a fixed 40x25 character grid. Each cell displays a single
glyph sourced from the page buffer. The grid scrolls vertically only through the
ORIGIN register -- there is no horizontal scroll and no pixel-addressable
graphics.

Character cells are arranged row-major: row 0 occupies columns 0-39, row 1
columns 40-79, and so forth. The display scans top-to-bottom, left-to- right, at
a rate determined by the hardware. This scan is visible to the operator if the
CPU writes during an active frame.

================================================================================ 3)
Page Buffer Layout
================================================================================

The Flight Core permits only 16-bit memory accesses. Byte writes are not
available, so the page buffer stores two characters per word to avoid
read-modify-write cycles on every character update.

Word layout:

- High byte (bits 8-15): even column (0, 2, 4, ...)
- Low byte (bits 0-7): odd column (1, 3, 5, ...)

Addressing a character at row R, column C:

- Byte index: i = (R \* 40) + C (0 to 999)
- Word index: w = i >> 1 (0 to 499)
- Byte select: s = i & 1 (0 = high, 1 = low)

The page occupies exactly 500 consecutive words in CPU memory. The base address
is configured via the PAGE_BASE register. Misaligned bases (odd addresses) or
bases that overlap MMIO or DIAG regions trigger a fault condition.

Rationale: Storing two characters per word means a straight write fills two
cells. This halves the number of memory operations compared to byte- addressable
designs, critical when every tick counts.

================================================================================ 4)
Character Interpretation
================================================================================

As the scan proceeds left-to-right within a row, each code is interpreted
according to the current line state (see Section 5). The interpretation is:

- Codes 0x20-0x7F: printable glyphs from the built-in ROM font
- Codes 0x00-0x1F: control codes that modify line state (not rendered)

Control codes take effect immediately upon encounter. They do not display a
glyph. If a control code appears at the end of a row, it has no visible effect
since the line state resets at the start of the next row.

================================================================================ 5)
Line State
================================================================================

Line state tracks styling attributes as the scan moves across a row. It
determines how each printable character appears. The state consists of:

- Foreground color: 0-7
- Background color: 0-7
- Mosaic mode: OFF / ON
- Flash mode: OFF / ON

At the beginning of each row, the state resets to defaults:

- Foreground = 7 (white)
- Background = 0 (black)
- Mosaic = OFF
- Flash = OFF

Control codes (Section 7) modify individual state components. The state persists
across the row and does not carry over to subsequent rows.

Rationale: Per-row reset ensures each line starts clean. A corrupted cell at
column 39 cannot pollute the next row -- the operator sees the damage where it
occurred, not cascading across the screen.

================================================================================ 6)
Mosaic Graphics
================================================================================

When Mosaic mode is ON, a subset of printable codes render as block glyphs
instead of standard ASCII characters. This enables simple UI graphics -- boxes,
progress bars, status indicators -- without pixel- level control.

The mapping is fixed and identical across all TELE-7 units. Available glyphs:

- Full block (solid fill)
- Left half block
- Right half block
- Partial fill blocks (suitable for progress bars)

Mosaic mode does not affect control codes (0x00-0x1F). Spaces render as spaces
regardless of mosaic state.

================================================================================ 7)
Flash (Blink) Mode
================================================================================

When Flash mode is ON, printable glyphs blink at a deterministic rate derived
from the system tick. This provides visual alerts without requiring the CPU to
repeatedly rewrite the display.

Blink timing:

- Tick rate: 100 Hz
- Blink phase toggles when an internal counter reaches BLINK_DIV
- Default BLINK_DIV = 50, giving approximately 2 Hz (500ms on, 500ms off)

During the OFF phase, the glyph foreground is suppressed -- the character
appears as background color only. Spaces remain spaces (no foreground to
suppress). Control codes do not blink regardless of flash state.

The blink phase is latched at the start of each scan and held for the entire
frame. This prevents mid-row phase changes that would create visually
distracting tearing within a single row.

Rationale: Tick-derived timing ensures all TELE-7 units blink in lockstep. A
crew debugging across multiple stations sees identical timing -- no confusion
about which blink cycle means "warning" versus "critical."

================================================================================ 8)
Sampling Modes
================================================================================

TELE-7 supports two modes for reading the page buffer:

LIVE_READ (default): The device reads page memory in real-time as it scans. If
the CPU writes during scan, the operator sees a mix of old and new content --
characters from before the write alongside characters from after. This is
visible "tearing" but reflects memory state directly.

LATCHED: The device snapshots the entire page at tick boundary and renders from
that snapshot for the duration of the tick. The operator sees stable content per
tick, but updates are delayed by up to one tick interval.

Use LATCHED when you need stable frames for logging or video capture. Use
LIVE_READ when you want minimum latency and accept the visual artifacts.

The mode is controlled via the LIVE_READ bit in the CTRL register (bit 1).

================================================================================ 9)
Control Codes
================================================================================

Control codes (0x00-0x1F) modify line state. They are not rendered as glyphs.
Unlisted codes are reserved and treated as no-ops.

Color codes: 0 Black 4 Blue 1 Red 5 Magenta 2 Green 6 Cyan 3 Yellow 7 White

Foreground color: 0x00-0x07 Set FG color to 0-7

Background color: 0x10-0x17 Set BG color to 0-7

Mosaic mode: 0x18 Mosaic ON 0x19 Mosaic OFF

Flash mode: 0x1A Flash ON 0x1B Flash OFF

Example: To display red text on a black background, emit 0x01 (FG=red) then 0x10
(BG=black) before the text characters.

================================================================================ 10)
MMIO Interface
================================================================================

All registers are 16-bit. Byte writes are not supported and will fault. The MMIO
window occupies 0xE120 through 0xE12F.

## Register Map

Addr Name Access Description

---

0xE120 ID RO Device identifier constant 0xE121 VERSION RO Revision (0x0001)
0xE122 CTRL RW Control bits 0xE123 STATUS RO Status flags 0xE124 PAGE_BASE RW
Base address of 500-word page buffer 0xE125 BORDER RW Border color (0-7 in low
bits) 0xE126 ORIGIN RW Row origin (0-24) for vertical scroll 0xE127 BLINK_DIV RW
Blink divisor (low byte, default 50)

## CTRL Register (0xE122)

bit 0: ENABLE 1 = device active, 0 = blank display bit 1: LIVE_READ 1 = live
scan, 0 = latched mode bits 2-15: reserved (read as 0, writes ignored)

## STATUS Register (0xE123)

bit 0: ENABLED reflects CTRL.ENABLE bit 1: PAGE_MAPPED page buffer is mapped and
valid bit 2: FAULT fault condition detected (see below) bit 3: BLINK_PHASE
current blink phase (0 = on, 1 = off) bits 4-15: reserved (read as 0)

## Fault Policy

If PAGE_BASE is misaligned (odd address), points to a forbidden region (MMIO,
DIAG, or non-existent memory), or overlaps reserved space, the device sets
STATUS.FAULT and renders a blank screen. The CPU is not faulted -- execution
continues. Clear the fault by writing a valid PAGE_BASE and acknowledging in
STATUS.

================================================================================ 11)
Software Patterns
================================================================================

Writing character pairs: Since each 16-bit word holds two characters, write both
simultaneously when possible. This avoids read-modify-write overhead.

; Write 'A' (0x41) to column 0 and 'B' (0x42) to column 1 ; Byte index 0 -> word
0, high byte -> value 0x4142 MOV [R0], #0x4142

Vertical scrolling: Use the ORIGIN register to shift the view without rewriting
the page buffer. Increment ORIGIN to scroll up (show lower rows), decrement to
scroll down. Write new content at the emerging edge.

Tick-bounded updates: A full 1000-cell page rewrite may span multiple ticks.
Design update loops to yield between chunks if latency matters. LATCHED mode
ensures each chunk commits at a clean tick boundary.

================================================================================ 12)
Notation Reference
================================================================================

R, C Row and column indices (0-indexed) i Byte index into page buffer (0-999) w
Word index into page buffer (0-499) s Byte selector within word (0=high, 1=low)
FG, BG Foreground and background color tick Basic time unit, 100 Hz (10 ms
period) BLINK_DIV Divisor for blink phase toggle

================================================================================
END OF SPECIFICATION
================================================================================
