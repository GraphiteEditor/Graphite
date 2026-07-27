# Brush engine: problem space

A growing list of the targets and requirements the brush engine must fulfill. This document
describes only the problem space — what must be true, never how it is achieved. Implementation
belongs in [`DESIGN.md`](DESIGN.md), whose decisions should cite the requirements they satisfy.

Rules for this document: entries get stable IDs (`R1`, `R2`, …) that are never renumbered or
reused; new discoveries are appended to the fitting section with the next unused ID; an entry
that turns out to be wrong is marked superseded (with a note why), not deleted.

## Rendering quality

- **R1 — Resolution independence.** A stroke re-renders at any zoom level or canvas resolution,
  changed at any time after it was drawn, with no loss of quality — the same guarantee Graphite
  gives vector data.
- **R2 — Reproducibility.** Re-rendering a stroke — after a zoom, in a new session, on another
  machine — produces the same image. Randomized brush behavior must be exactly reproducible;
  numeric rendering differences across GPUs must stay below perceptual relevance.
- **R3 — Self-overlap is a brush property.** A translucent stroke of a solid brush covers itself
  uniformly (no darkening where it crosses itself); a build-up brush (airbrush) deliberately
  accumulates. Both behaviors must be expressible, per brush.
- **R4 — Smooth at any magnification.** Stroke edges show no aliasing or pixelation at any zoom;
  hairline-thin strokes remain visible and stable rather than shimmering or vanishing.
- **R5 — Correct compositing.** Strokes participate in the document like any other content:
  layer order, opacity, and blend modes apply; results match what the artist saw while drawing.

## Input

- **R6 — Full stylus expressiveness.** Pressure, tilt, barrel rotation, and timing are
  first-class inputs, and the design must not preclude further axes hardware may offer
  (e.g. tangential pressure).
- **R7 — Graceful degradation.** Mouse and touch input without stylus axes must still produce
  good strokes; missing axes degrade to sensible behavior, never to broken rendering.
- **R8 — Input as produced.** Devices deliver events at 60–1000+ Hz, irregularly spaced, with
  jitter. The engine accepts the trace as the hardware and the artist produced it; it does not
  require regularized input.
- **R9 — Real-time feedback.** The stroke appears under the pen while drawing, at latency low
  enough for hand–eye control — target: on par with established painting applications.
- **R10 — What you saw is what is kept.** When smoothing/stabilization is active during drawing,
  the stroke the artist watched appear is the stroke that persists — no surprise reshaping
  afterwards.

## Brush expressiveness

- **R11 — Three brush families.** The engine supports texture-stamp brushes, fully
  mathematically defined brushes, and brushes that wrap a texture along the stroke.
- **R12 — Parameters vary along the stroke.** Size, flow, rotation, and other brush parameters
  can change continuously over the course of a single stroke.
- **R13 — Dynamics are configurable and re-editable.** How input axes (pressure, tilt, speed, …)
  drive brush parameters is user-configurable — including after the stroke was drawn, applied
  retroactively without loss.
- **R14 — Controlled randomness.** Brushes may use randomized behavior (scatter, jitter, texture
  variation); it must be exactly reproducible on re-render (ties to R2).
- **R15 — Color integration.** Stroke color comes from the document's styling system. Per-point
  color effects (mixing, wet edges) are a plausible future and must not be ruled out.

## Non-destructive editing

- **R16 — Strokes are ground truth.** What the artist drew is stored and is never mutated by
  rendering, zooming, or exporting.
- **R17 — Everything re-derives.** Any brush setting changed after the fact re-renders affected
  strokes from their stored data.
- **R18 — Strokes are document objects.** They can be selected, moved, transformed, reordered,
  deleted, and all of it undone — like any other Graphite content.
- **R19 — Erasing exists.** Artists can remove paint with eraser strokes without destroying the
  underlying stroke data's editability.

## Document & integration

- **R20 — Practical persistence.** Strokes persist in Graphite documents; a painting with
  thousands of strokes and millions of input samples must load, save, and stay reasonably sized.
- **R21 — Node-graph native.** Strokes are ordinary values in the node graph, renderable and
  transformable by nodes, composable with the rest of the graph like vector data.
- **R22 — Adaptive resolution.** Rendering cooperates with Graphite's adaptive resolution
  system: only what is visible is rendered, at the resolution the view actually needs.

- **R26 — Stability across engine evolution.** Improving or fixing engine algorithms (smoothing,
  dynamics semantics, derived-signal formulas) must never silently reshape strokes in existing
  documents; old strokes keep rendering as they did or are migrated explicitly.
- **R27 — Live preview equals committed result.** The stroke rendered while drawing and the
  re-render after commit (or after load) are the same image — not approximately, but by
  construction; the artist never sees their stroke change at pen-up or on reopen.
- **R28 — Sculpting composes with dynamics.** Hand-authored per-sample adjustments (e.g. a width
  bulge sculpted onto a finished stroke) and retroactive dynamics edits must be able to coexist
  on one stroke without destroying each other.

## Performance

- **R23 — Interactive at scale.** Panning and zooming a painting with thousands of strokes stays
  at interactive frame rates.
- **R24 — Incremental drawing.** Continuing a stroke must not require re-rendering the whole
  painting — or the whole stroke — for every input event.
- **R25 — Bounded memory.** CPU and GPU memory use does not grow without bound in stroke count,
  stroke length, or zoom depth.
