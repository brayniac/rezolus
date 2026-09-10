# Diagrams must encode category in style, not only in label

- **Opened:** 2026-09-02
- **Status:** **IMPLEMENTED** for the one diagram the repo has
  (`docs/architecture.dot`, PR #1134). The convention below is the durable
  part; the diagram change is just its first application.
- **Arc:** none — a standalone convention, recorded because it was learned
  from a specific defect rather than adopted from style guidance.
- **Owner:** Yao Yue
- **Repos:** rezolus (`docs/architecture.dot`, `README.md`,
  `site/docs/architecture.html`).

## Why

`docs/architecture.dot` drew **Target systems** and **Target services** with
the same rounded-filled box as `rezolus agent`, `rezolus record` and the rest.
Colour differed — green against pink and blue — but colour was already doing
another job in that diagram (it groups by *stage*: what you watch, what
collects, what you do next), so the one distinction a newcomer most needs was
carried by nothing except the word "Target" in the label.

The caption made it worse rather than better. It read *"Every box is the same
`rezolus` binary — you just pick the subcommand for the job."* That was never
true of those two boxes, and a caption asserting a uniformity the diagram does
not have is worse than no caption: it actively teaches the wrong model to the
reader least able to catch it.

## The convention

**When a diagram contains items from two categories the reader must not
conflate, encode the difference in a style channel, not only in a label or a
position.**

Applied here: dashed outline for things outside the system being described,
solid for things inside it. The same rule covers the other pairs that keep
coming up — ours / theirs, known / inferred, stable / experimental, measured /
modelled, synchronous / asynchronous.

Three things that make it work:

1. **A label is read only by someone already looking at that box; a style is
   read at a glance.** The distinction that governs how the whole picture is
   understood belongs in the channel that survives skimming.
2. **State the encoding in the caption.** A dashed box means nothing on its
   own. The caption says which channel carries which meaning, and it must
   describe what the diagram actually shows, not what it mostly shows.
3. **Style channels are scarce — check what is already spoken for.** This
   diagram already used dashes for an *edge* (`service -> record`, "scrape"),
   so dashes now carry two meanings. Boxes and arrows are distinct enough that
   it reads fine, but that was luck rather than design, and a third dashed
   thing would break it. The channels available are outline style, fill,
   colour, shape, line weight and size; before assigning meaning to one, look
   at what the diagram already spends it on.

## Why this repo in particular

Rezolus already refuses to present an inferred number as a measured one — that
is the whole point of acquisition windows and `rate()` uncertainty bands, and
principle 16 refuses "low overhead" as a claim with no measurement behind it.
Rendering *what we measure* identically to *what does the measuring* is the
same category error in a different medium. A diagram is an interface to the
system's model of itself, and it should be held to the honesty the code is
held to.

## Outcome

`docs/architecture.dot`: both target nodes take `style="rounded,filled,dashed"`.
The captions on `site/docs/architecture.html` and in `README.md` now say what
solid and dashed mean instead of claiming every box is the binary. Regenerated
with `dot -Tsvg docs/architecture.dot -o docs/architecture.svg`;
`site/docs/architecture.svg` is a symlink, so the page follows.

## Deferred or reopen items

- ~~The repo has exactly one diagram today, so the convention has a sample size
  of one. *Reopen* when a second is added — that is when it will be clear
  whether the rule needs a shared palette or legend, and whether dashes should
  be reserved for external-ness alone.~~ **Answered 2026-09-10**, by a set of
  three in `iopsystems/dendro` (see
  [the dendro extraction](2026-09-10-dendro-container-extraction.md)). Both
  questions came back yes, and the second came back the hard way.

  **A shared palette and a per-chart legend are both needed.** Three charts of
  one system have to read as one system, which means one module owning the
  palette and the glyph primitives. The legend has to be drawn by that same
  generator: a key assembled from text or table cells can approximate a fill
  but not a silhouette, and an approximated shape teaches a glyph the chart
  never uses.

  **Dashes should be reserved for external-ness alone.** The first draft spent
  the channel three ways — "outside dendro", "in memory", and "has no catalog
  row" — and the result was that dashes meant nothing at all, which was only
  visible on looking at the three charts side by side. The fix was not a better
  legend; it was giving two of those claims other homes: one moved into the
  node's own label, and one turned out to be the wrong claim anyway (the file
  had been drawn as a node beside its own contents, and became the enclosing
  cluster instead).

  That is the convention's own rule biting one level up. "Encode category in a
  style channel, not only in a label" is necessary but not sufficient — the
  channel also has to carry **exactly one** category, and across the whole set
  rather than per chart. A channel with two meanings is worse than a label,
  because a label at least admits it needs reading.

  One addition worth carrying forward: the convention says to state the
  encoding in the caption, and that survived contact, but a *generated* set
  wants the encoding in a generated key as well. The caption says what the
  chart is about; the key says what the marks mean, in the marks themselves.

## Appendix: Skills Invoked

- `engineering-journal` — opened this entry and its index row.
