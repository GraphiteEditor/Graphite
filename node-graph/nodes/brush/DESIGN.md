# Brush engine: stroke representation

Design notes for the GSoC GPU brush engine. This document covers how strokes are *stored* — the
resolution-independent source format — and the core types in [`src/types.rs`](src/types.rs).
Renderer internals (WGSL, tiling, compositing) are out of scope here and will be designed per
renderer family on top of these types. The existing nodes in this crate are throwaway prototypes;
nothing here is derived from them.

## Requirements

The problem space — the growing list of targets and requirements this design must satisfy —
lives in [`REQUIREMENTS.md`](REQUIREMENTS.md) and is referenced from here by ID (R1, R2, …).
The stroke format below is chiefly shaped by R1 (resolution independence), R2/R14
(reproducibility), R6–R8 (input fidelity), R11–R13 (brush expressiveness), R16–R17
(non-destructiveness), and R20–R21 (persistence and node-graph integration).

## Prior art

**MyPaint** — records raw input events (position, pressure, time delta); a brush is ~50 settings,
each optionally *mapped from input axes* (pressure, speed, direction, tilt declination/ascension,
random, stroke progress) via curves, and dabs are derived from that. The canvas itself is
destructive, but the recording model shows raw events are compact and sufficient. Lesson: store
raw axes; expressiveness lives in the input→parameter mapping, not in the samples.

**Krita** — each input event is a `KisPaintInformation`: position, pressure, tilt X/Y, barrel
rotation, speed, time. Brush engines read these through "sensors" (pressure, speed, distance,
fade, drawing angle, fuzzy…) shaped by user curves into size/flow/rotation/scatter/etc. Dab
placement walks arclength using per-stroke distance state. Same lesson as MyPaint, plus:
spacing is an arclength-resampling problem, separate from the stored samples.

**Ciallo (Ciao et al., SIGGRAPH 2024)** — GPU vector strokes: a stroke is a *polyline with
per-vertex attributes* (radius, color, …), and three fragment-shader renderers consume that one
representation — "vanilla" (capsule SDF), "stamp" (per-pixel analytic dab evaluation), and
"airbrush" (closed-form Gaussian sweep). This is the strongest evidence that an attributed
centerline is a sufficient interchange for all our renderer families at interactive rates, and
that strokes can stay vector-form all the way to the fragment shader. Its per-vertex attributes
are exactly our channels.

**Photoshop / Procreate** — the stamp-brush semantics we adopt: *flow* is per-deposition paint
rate (builds up within a stroke), *opacity* is a per-stroke ceiling. Procreate's "rolling grain"
(texture advancing continuously along the stroke) is essentially our ribbon family. Both bake to
a fixed-resolution canvas — exactly the limitation this engine avoids.

**Inkscape PowerStroke** — variable width stored as pure path geometry: proof that a width
channel is resolution-independent vector data, but it has no flow/texture/dab concept, so it
covers only the geometry half of the problem.

**Game-engine trail ribbons** (Unity TrailRenderer, Spine, etc.) — the standard construction for
our ribbon family: extrude the centerline into a triangle strip, U = arclength / repeat length,
V across the width. Well-trodden; the open problems are joint folding and self-overlap blending,
which are renderer concerns, not storage concerns.

## Decisions

### D1 — A stroke is pure data; brush behavior lives in nodes

`Stroke` stores only the sampled centerline and its channels. Everything that makes a brush *a
brush* is a node consuming that data:

```
Stroke data  →  dynamics node  →  renderer node (stamp | procedural | ribbon)  →  Raster<GPU>
                (pressure→width,                (spacing, hardness, textures, …
                 speed→flow, …)                  as node inputs)
```

This is the maximally Graphite-native factoring: a brush preset is a node-chain configuration,
so every brush parameter is live-tweakable after the stroke is drawn, dynamics remapping is
non-destructive and visible in the graph, and the stored format stays renderer-agnostic. It also
means `types.rs` contains no `Brush` enum and no dynamics struct — the three families differ in
*how they consume* the same channels, which is node-signature territory, not data-format
territory. (A serialized preset type for the tool UI comes later and is orthogonal.)

### D2 — Two channel groups: raw input axes and render channels

The channels on a stroke split by role:

| channel                            | written by                       | read by            |
| ---------------------------------- | -------------------------------- | ------------------ |
| `position`                         | tool (capture)                   | everything         |
| `pressure`, `tilt`, `twist`, `time`| tool (capture) — never mutated   | dynamics           |
| `width`, `flow`, `rotation`        | dynamics — or baked / sculpted   | renderers          |

Raw axes are the captured device data — ground truth for any future re-derivation. Render
channels are what renderers consume: conventionally the output of a dynamics stage, but real
storage, so a simple pipeline can bake them at capture time and a future tool can hand-sculpt
them (e.g. a width bulge on a finished stroke). The contract that keeps this coherent: renderers
never read raw axes, and dynamics never mutates them.

This dissolves the classic raw-vs-baked dilemma (MyPaint/Krita store raw; Ciallo stores baked)
by storing both cheaply — see D3 for why unused channels cost nothing.

### D3 — Channels are `Uniform(T) | Samples(Vec<T>)`, stored per attribute (SoA)

Every per-sample attribute is a `Channel<T>`: either one uniform value or one value per sample.

- **No fabricated data**: a mouse stroke has `Uniform` pressure; a stylus without tilt reports
  `Uniform` tilt. "This stroke has no per-sample signal here" is representable, not faked with
  vectors of identical values.
- **Cheap storage**: strokes will be the bulkiest data in a painted document; a channel that
  carries no signal costs one value, both in memory and serialized.
- **Statically visible uniformity buys faster renderers**: a uniform-width stroke can be drawn
  as a union of equal-radius capsules (a min-distance evaluation); variable width needs
  per-segment cone/trapezoid handling. Uniform flow admits closed-form accumulation the way
  variable flow does not. The renderer sees which case it is in without scanning the data.
- **SoA is the GPU shape**: per-attribute arrays upload as contiguous buffers and interpolate
  per channel; there is no struct-of-everything to pad or split.

Sample count is defined by `position` (the only mandatory per-sample data); every `Samples`
channel must match its length (`Stroke::is_valid`).

### D4 — Document space, `f64` positions, document units for lengths

Positions are `DVec2` in document space; `width` is a diameter in document units. Scalar
channels are `f32` (their precision needs are trivial); positions are `f64` because document
coordinates can be large and strokes must survive deep zoom — rebasing into a local `f32` space
happens per render, against the current footprint, and is lossy only for that frame.

Transform interplay: like `Vector`, a stroke item can carry `ATTR_TRANSFORM`. Uniform scale must
scale `width` alongside positions. Non-uniform scale of a brush stroke is ill-defined (a round
dab has no correct ellipse orientation under shear); initially it applies to positions only,
with width scaled by the geometric mean of the axes.

### D5 — Samples are the trace as drawn, at capture density

Samples are stored at input-event density (typically 60–1000 Hz), irregularly spaced, *after*
any capture-time smoothing/stabilizer — we persist what the artist saw, not the jittery
pre-stabilizer device data. Capture-time decimation (dropping collinear/duplicate events within
tolerance) is fine; after storage the samples are ground truth and are never resampled in place.

Resampling is a render-prep concern per family: stamps walk arclength at `spacing × width`,
ribbons tessellate vertices, procedural sweeps use the polyline directly. None of that derived
data is stored. The canonical interpolation between stored samples (linear, shortest-arc for
angles) is encoded once, in `Stroke::sample_lerp`, so every renderer resamples identically.

### D6 — Determinism via a stored per-stroke seed

Randomized brush behavior (scatter, jitter, per-dab texture rotation) must reproduce exactly on
every re-render — resolution independence is meaningless if zooming reshuffles the dabs. Each
stroke stores a `seed`; all randomness in dynamics and renderers derives from it (plus stable
per-dab indices), never from global RNG state.

### D7 — Node-graph integration via `List<Stroke>`

Strokes flow as `List<Stroke>`, mirroring `List<Vector>`: color comes from the item's paint
attributes (`ATTR_FILL`/`ATTR_STROKE`) rather than being embedded per stroke, and
`ATTR_TRANSFORM` applies per D4. Renderer nodes are `List<Stroke> → List<Raster<GPU>>`. A layer
renders all its strokes through one brush chain; per-stroke parameter variation within a list
(mixed brushes on one layer) is a later question, most naturally answered with per-item
attributes.

## Render channel semantics

What each renderer family does with the render channels at a point on the centerline:

| channel    | unit                | stamp                        | procedural                    | ribbon                          |
| ---------- | ------------------- | ---------------------------- | ----------------------------- | ------------------------------- |
| `position` | document space      | dab placement (via spacing)  | polyline vertex               | strip spine vertex              |
| `width`    | document units, ⌀   | dab diameter                 | local sweep width             | strip width                     |
| `flow`     | 0..=1               | per-dab alpha                | local deposition density      | local texture alpha             |
| `rotation` | radians (+X, Y-down)| stamp orientation            | footprint orientation (if any)| twist added onto tangent frame  |

## Open questions

- **Dynamics node design**: which input axes map to which render channels, through what curves;
  MyPaint's mapping table and Krita's sensors+curves are the references. This is the next design
  round and touches only node parameters, not the stroke format.
- **Brush preset type**: a serializable bundle of dynamics + renderer settings for the tool UI.
  Orthogonal to the format; likely lives near the tool, not in this crate.
- **Per-sample color/wetness**: color mixing and wet-edge effects would add render channels.
  The `Channel` mechanism makes that additive; deferred until a renderer needs them.
- **Opacity ceiling**: flow is per-sample; the Photoshop-style per-stroke opacity cap is a
  compositing parameter — likely a stroke- or layer-level attribute, not a sample channel.
- **Eraser strokes**: probably a stroke-level flag or blend-mode attribute; interacts with
  compositing design, not with the sample format.
- **Tilt representation**: stored as projected tilt-X/tilt-Y angles (the PointerEvent model,
  what tablets report); dynamics that want altitude/azimuth derive them. Revisit when the winit
  tablet API lands and dictates what we actually receive.
- **Serialization**: these types need `serde` derives (behind a feature, as in sibling crates)
  once strokes are persisted in documents; the crate has no serde feature yet. `Channel`'s
  enum form serializes compactly by construction.
- **Editing identity**: `Vector` gives every point a stable ID for modification tracking.
  Strokes are append-mostly and edited as wholes, so per-sample IDs are omitted; if partial
  stroke editing (split/trim) arrives, IDs or spans get revisited.
