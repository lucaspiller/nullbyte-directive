# Technical Writing Guide

Style and formatting rules for in-game technical documents (specs, manuals,
compliance docs).

## Voice

These documents are written by engineers who survived the Quiet Burn. They are
precise but not cold. They explain _why_, not just _what_. The reader is someone
who will spend hours programming these machines -- they deserve to understand
the designer's intent.

Rules:

- Write to a competent person, not a compliance parser.
- Use direct statements instead of RFC-style normative language. Say "the core
  faults" not "the core MUST fault." Reserve MUST and MUST NOT for
  certification-critical callouts only.
- Every section should open with 1-3 sentences of prose before any tables or
  lists. Explain what the section covers and why it matters.
- If a design choice has a reason, state it. "The queue is 4 entries deep
  because a deeper queue would cause unpredictable processing spikes" is better
  than "Depth: 4."
- Personality is allowed but earned. One sharp line per document is worth more
  than ten. "A halted core is a safe core" works. Jokes in every paragraph do
  not.

## Formatting

All technical docs use plain ASCII with an 80-character line limit.

### Line width

No line may exceed 80 characters. This includes table rows, separator lines, and
code examples. No exceptions.

To reflow a plain-text file after editing:

```
fmt -w 80 <file>
```

This wraps paragraphs to 80 characters and preserves blank-line paragraph
breaks. Review the output before overwriting -- `fmt` does not understand tables
or indented blocks, so check those by hand.

### Section headers

Use 80-character `=` separator lines above and below:

```
================================================================
SECTION TITLE
================================================================
```

For numbered sections:

```
================================================================
3) SECTION TITLE
================================================================
```

### Tables

Use fixed-width columns aligned with spaces. Separate header from body with
dashes. Keep column count low -- if a table needs more than 4 columns, consider
splitting it.

```
Name    Width  Meaning
------  -----  ----------------------------------------
R0..R7  16     General purpose
PC      16     Program counter
```

### Indented blocks

Use 2-space indentation for sub-items under a heading:

```
Commit order:
  1. Read source operands
  2. Compute result
  3. Perform memory writes
```

### Code examples

Use assembly-style examples with comments. Indent with 2 spaces from the
surrounding context:

```
  MOV  R0, #0x4000   ; base address
  MOV  R1, #0x0010   ; offset
  CALL #add_offset   ; R0 now holds 0x4010
```

Place examples near the concept they illustrate, not in a separate section at
the end.

### Dashes

Use `--` (double hyphen) for parenthetical breaks. Do not use unicode em-dashes
or en-dashes.

```
Good: small enough for a crew to audit -- what you see is what
      you get.
Bad:  small enough for a crew to audit — what you see is what
      you get.
```

### Emphasis

No markdown formatting (bold, italic, headers with `#`) inside technical spec
documents. These are plain-text files. Use CAPITALISATION or "quotes" sparingly
for emphasis. Structure and word choice should carry the weight.

Design docs (like this one) may use markdown since they are not in-game
artifacts.

## Structure

### Preamble

Every spec starts with a short preamble (3-6 sentences) that answers:

- What is this thing?
- Why does it exist?
- What principle drove its design?

This is the reader's first contact. It should be inviting, not a wall of bullet
points.

### At-a-glance

A short summary (8 lines or fewer) of the key facts someone needs to hold in
their head. No tables, no acronym soup, no normative keywords.

### Body sections

Number them sequentially. Each section should:

1. Open with prose (what and why)
2. Present the reference material (tables, rules, layouts)
3. Include at least one example if the section describes behavior

### Notation

If the document uses shorthand, collect it in a reference section at the end --
not the beginning. Readers don't memorise notation tables; they refer back to
them. Introduce each shorthand naturally at first use where possible.

### Closing

End with a clear terminator:

```
================================================================
END OF SPECIFICATION
================================================================
```

## Content rules

### In-game framing

Technical specs are in-game documents. They must not reference:

- MMO mechanics, simulation, game servers, or player counts
- Out-of-world implementation details
- Real-world computing history or comparisons

The "why" comes from the Exodus Protocol world: the Quiet Burn, inspectability,
computational poverty, voter trust, the Laminated Stack. Use that vocabulary.

### Design rationale

State the reason for important constraints near the constraint itself. Good
places for rationale:

- After a table that defines limits (queue depth, tick budget)
- In the opening prose of a section
- As a closing sentence after a rule

One sentence is usually enough. Do not write essays.

### Examples

Examples should be:

- Short (3-6 lines of assembly, max)
- Placed inline near the relevant rule
- Commented to explain what is happening
- Realistic (something a programmer would actually write)

Aim for at least 3-4 examples per specification document, distributed across the
sections that describe behavior (execution, dispatch, events, calling
conventions).
