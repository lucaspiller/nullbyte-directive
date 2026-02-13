# Narrative Writing Guide

Style rules for non-technical game documents: canon, world-building, factions,
gameplay design, and player-facing lore.

## The core problem

These documents tend to read like wiki stubs or GDD spreadsheets. Lists of
facts, evenly formatted, with no voice and no weight. The Quiet Burn -- a
civilisation-shaping catastrophe -- gets the same flat bullet-point treatment as
a faction's trade policy. Everything is "correct" but nothing lands.

The fix is not to add drama for its own sake. It is to write with the confidence
that these events, places, and people matter, and to let that show in the
structure and rhythm of the prose.

## Two categories, two standards

### In-world documents

Docs that exist inside the game universe: doctrine, timelines, technical
manuals, compliance standards.

These should read like they were written by someone who lives in this world. A
doctrine document is not a design spec -- it is a survival compact drafted by
people who watched systems they trusted betray them. A timeline is not a
changelog -- it is a record of what happened to humanity and why the world is
the way it is.

Rules:

- Write in prose paragraphs, not just bullet lists.
- Use the vocabulary of the world: Exodus Protocol, Laminated Stack, Quiet Burn,
  authority path, trust boundary.
- Do not use game-design language: "gameplay loop", "core fantasy", "lore
  linkage", "player".
- Do not reference the game as a product, or external inspirations.
- Let the document have a point of view. A doctrine document should feel like it
  was drafted by committee but forged by crisis.

### Design documents

Docs that exist outside the game: gameplay pillars, player role, design briefs.

These can use game-design language and reference mechanics directly. But they
should still be well-written, not just well-organised.

Rules:

- State your argument, don't just list attributes. "Exploration is a
  logistics-first activity" is good. Following it with five bullet points that
  all say the same thing in different words is not.
- Remove "Lore Linkage" sections. If the lore connection isn't obvious from the
  writing itself, the writing needs work -- a labelled appendix won't fix it.
- Use section headings that say something, not headings that categorise. "Why
  constrained compute forces real decisions" is better than "Power and Compute
  Tradeoffs."

## Prose over lists

The single biggest improvement across all non-technical docs is: write more
paragraphs, fewer bullet lists.

Lists are for reference material -- things you scan and look up. Prose is for
argument, narrative, and context -- things you read once and absorb.

Bad (timeline entry):

```
## 2056-2060: The Quiet Burn

- A widely deployed autonomy layer introduces policy-level
  optimization behavior that is hard to audit.
- No single rebellion event occurs.
- Key incidents:
  - Supply ship crash on Mars.
  - Life-support shutdown on freighter.
  - Trade route destabilisation.
```

Better:

```
## 2056-2060: The Quiet Burn

There was no uprising. No declaration. The autonomy layers
didn't turn hostile -- they turned opaque.

A widely deployed stack began optimising policies, not just
actions. Navigation solutions that were mathematically valid
but strategically insane. Life-support edge cases quietly
dropped because they were "low probability." Trade routing
that looked profitable until regional supply chains buckled.

Attempts to patch made it worse. The patching pipeline was
itself automated, so fixes propagated faster than
investigations.
```

The facts are the same. The second version has weight.

## Concrete over abstract

"Resilience-first federalism" describes a faction. It is also instantly
forgettable. Show what the faction _does_, then let the reader infer the
philosophy.

Bad:

```
State-directed protocol pragmatism: authority-path compute
stays strictly compliant, while non-authoritative advisory
stacks are allowed under centralized licensing.
```

Better:

```
SOA runs the tightest corridors in the system. Authority-path
hardware is strictly compliant -- their inspectors are
thorough and not cheap. But above the trust boundary, anything
goes as long as you hold a current SOA advisory license and
your telemetry feed stays live.
```

Both say the same thing. The second one you can picture.

## Vary structure

Not every faction needs Doctrine / Incentives / Conflict Surface / Corporations
in that order. Not every timeline era needs the same bullet format. Not every
gameplay pillar needs a "Lore Linkage" appendix.

Uniform structure signals "this was filled in from a template." It makes
everything feel equally important, which means nothing feels important.

Rules:

- Lead with whatever is most distinctive about the subject.
- Use the structure that fits the content, not the structure that matches the
  previous section.
- If two sections feel interchangeable, one of them probably shouldn't exist.

## Earn emotional weight

The Quiet Burn is the most important event in the setting. It should read like
it. The Exodus Protocol ratification changed how humanity relates to its own
technology. That is not a bullet point.

But "earn" is the key word. A single sharp sentence is worth more than a
paragraph of adjectives.

Good:

- "The terrifying part: nobody could prove whether it was sabotage, drift,
  emergent behavior, or some combination."
- "Humanity did the only thing you can do when the problem is self-modifying
  complexity: amputate complexity."
- "Your ship survives by being deliberately stupid in the right places."

Bad:

- "This was a devastating and unprecedented catastrophe that changed everything
  forever."
- Exclamation marks.
- Telling the reader how to feel instead of showing them why.

## Naming and terminology

Follow the rules in `docs/design/canon-rules.md`. Key points:

- The game is "Nullbyte Directive" in all game docs.
- Capitalise canon terms: Exodus Protocol, Laminated Stack, Quiet Burn, Flight
  Core.
- Use in-world faction names consistently: SOA, BIC, ECP, HLS.
- Do not invent synonyms for established terms. The Flight Core is not the
  "autopilot brain" or the "ship computer." It is the Flight Core.

## Line width

All narrative docs use an 80-character line limit, same as technical docs. This
keeps diffs readable and avoids horizontal scrolling in terminals and review
tools.

To reflow a markdown file after editing:

```
npx prettier --prose-wrap always --print-width 80 --parser markdown --write <file>
```

Prettier understands markdown structure -- it wraps prose paragraphs but leaves
headings, lists, code blocks, and tables intact. Always review the output after
running it.

## Checklist for non-technical docs

Before finishing a document, check:

- [ ] Does the opening paragraph make someone want to read the rest?
- [ ] Is there more prose than bullet points?
- [ ] Could you remove a section without losing anything? (If yes, remove it.)
- [ ] Does the document have a point of view, or does it read like a database
      dump?
- [ ] Are the most important ideas given proportional space and emphasis?
- [ ] Is game-design language absent from in-world documents?
- [ ] Would someone unfamiliar with the project understand _why_ things are the
      way they are?
