# Rank Polymorphism Implementation

Status and staging plan for the rank polymorphism refactor. The classification that drives this work is in `rank-polymorphism-node-audit.md`.

## Implemented (this branch)

### Element-wise kernels, declared structurally by `Item<T>`

A node whose primary input is typed `Item<T>` is an element-wise rank-0 kernel: it consumes one item per call, with full access to the item's element and attributes. No macro attribute is needed; the signature is the declaration. The macro generates two registry variants per implementations row under the same node identifier:

- **`Item<T>` variant** (struct `XNode`): evaluates the primary input as one `Item<T>` and calls the kernel once. Output wire: `Item<U>`. Dormant until rank-0 wires flow (generator flip stage), but registered and type-checked now.
- **`List<T>` variant** (struct `XNodeMapped`): evaluates the primary input as a `List<T>` and calls the kernel once per item, broadcasting all other parameters by clone. Output wire: `List<U>`. This exactly replaces today's hand-written per-row loops, so existing documents resolve to it with identical behavior.

The kernel owns the item wrapping on both sides: it receives `Item<T>`, may read or write attributes, and returns `Item<U>`. Generic element-wise nodes list bare element types in `#[implementations(...)]` (e.g. `Graphic, Vector, Raster<CPU>`); the macro derives both wire forms. Field metadata, `NodeInputDecleration` accessors, and default types keep the `List` wire form for document compatibility.

Rules enforced by validation: only the primary input may be `Item`-typed; an `Item<T>` primary requires an `Item<U>` return and bare-element implementations; an `Item<U>` return requires an `Item<T>` primary. Bare-typed primaries continue to mean today's plain wires, so unmigrated nodes are untouched. `shader_node` kernels are now compatible: the same `Item<T>` body is re-emitted against a transparent GPU stand-in (see below).

Migrated so far (~120 nodes): the graphical element-wise batch (Bounding Box, Close Path, Blur, Median Filter, etc.), the attribute-manipulating nodes (Blend Mode, Opacity, Clipping Mask, Reset/Replace Transform), Transform itself (including the flagship lazy-primary zip that broadcasts `Item<Vector>` content across a `List<DVec2>` frame), the string and math families, the expanders (string split, regex, JSON query, Separate Subpaths, Image Color Palette), and the raster adjustment/blending/GPU chunk (16 adjustments plus Mix, Color Overlay, Gradient Map). The attribute-manipulating nodes became one-line kernels (`content.set_attribute(ATTR_BLEND_MODE, blend_mode)`), which deleted the ~180 lines of `SetBlendMode`/`MultiplyAlpha`/`MultiplyFill`/`SetClip` trait impls in the blending crate and resolved its three "apply once to the list's parent" TODOs — the `Item` variant is precisely that path.

### Supporting changes

`Item<T>` implements `StaticType`, making it a legal type-erased wire type.

### Framed Switch and whole-collection bundling

Switch is a framed element-wise select. Its condition is a ranked `Item<bool>` primary; its two branches are lazy `Context -> Item<T>` connectors. A single bool picks one branch's item (the Item variant); a `List<bool>` frames per slot, evaluating only the selected branch each slot, so short-circuit laziness holds. A stored bare bool resolves to the Item variant unchanged, so no migration is needed.

Whole-`List<X>` branches (a layer stack, or any list of scalars or values) ride a `Bundle<T>(List<T>)` newtype, letting an entire collection flow as one rank-0 `Item<Bundle<X>>` cell. Two structural promotion arms bundle a `List<X>` wire into the branch and unbundle the result back to a flat `List<X>` at its consumer; both are confined to the `Bundle` tag, which appears only on Switch's branches, so nothing else re-ranks. This restores whole-stack switching while keeping each stack opaque, so a `List<bool>` never zips per-element inside the two stacks. The bundle/unbundle adapters forward their eval context, so composing them onto a lazy branch preserves laziness. The promotion machinery treats lazy and eager connectors alike; the branch resolved before only because no `List<X> -> Item<Bundle<X>>` direction existed, not because it is lazy.

## Authoring guide

```rust
#[node_macro::node(category("Blending"))]
fn blend_mode<T>(
	_: impl Ctx,
	#[implementations(Graphic, Vector, Raster<CPU>, Color, Gradient, String)]
	content: Item<T>,
	blend_mode: Item<BlendMode>,
) -> Item<T> {
	let mut content = content;
	let blend_mode = blend_mode.into_element();

	content.set_attribute(ATTR_BLEND_MODE, blend_mode);
	content
}
```

Owned parameters are never declared `mut` in the signature; shadow them with `let mut` at the top of the body. Ranked non-primary parameters are `Item`-typed too and are unwrapped with `into_element()` before use.

Kernels that ignore attributes simply pass them through by mutating the element in place (`content.element_mut()`) or via `into_parts`/`from_parts` when the element type changes. Bare-`T` kernel sugar (macro-owned attribute plumbing) is deliberately deferred until the system is proven; `Item<T>` everywhere is the one authoring style.

## Staged roadmap

1. **Batch migration (mechanical, no new machinery).** LANDED. All audit-classified element-wise nodes are converted to `Item<T>` kernels, ending with Combine Channels (ranked channels under its `()` primary), Map Points, and the Upload Texture deletion. Two conversions are deliberately deferred by design decision, not omission: Fill's paint connector (needs zip-aware document wiring to replace its whole-`List` fill collapse) and To Graphic (pending the auto-conversion subsumption decision). Path Modify converts at stage 4, where its empty-input synthesis and the layer coercion path become rank-0 capable; Extract Transform converts at stage 7 once the vararg readers emit `Item` wires.
2. **Rank-aware resolution and promote adapters (compiler).** LANDED. The typing context retries failed resolutions with per-input promotion adapters (`WrapItemNode` bare -> `Item`, `ItemToListNode` `Item` -> singleton `List`, `WrapListNode` bare -> `List` at double cost, `UnwrapItemNode` `Item` -> bare for legacy connectors), picking the variant with the cheapest total promotion cost and leaving ties ambiguous. Promotions are recorded per node and applied by the executor at construction time as argument wrappers, so the graph structure itself stays adapter-free. The preprocessor's generated wrapper networks route ranked fields through `FieldAdapterNode` (wrap or sanctioned element conversion) instead of `IntoNode`/`ConvertNode`.
3. **Multi-connector zip.** LANDED. The macro's mapped variant zips every ranked connector by frame slot (longest-list, last-element repeats), broadcasts bare parameters by clone, and stamps the slot index onto the context for lazy connectors, which re-evaluate per slot; this is what delivers `Item<Vector>` content x `List<DVec2>` translations -> `List<Vector>` through Transform, and it retired the hand-written Transform-zip, Area, and Centroid companions. Expander kernels flat-map under the frame per the rank-2 force-flatten rule. The lazy-primary list-content variant complements it by framing over the content length with a precomputed per-slot stub. Attribute precedence needs no merge step: the kernel writes through its primary item's attributes.
4. **Generator flip + document migration.** LANDED. Every generator emits `Item<T>` (the vector shapes, value/color/text, raster, web-request, and context-reader families), the frontend displays `Item<T>` as `T`, and rank-0 wires flow through real documents. The layer coercion path (`to_graphic`, `wrap_graphic`, `extend`) needed no rank-0 signature forms: the promotion machinery raises an `Item` content wire at `wrap_graphic`'s `List` connector, so a rank-0 chain composes through every layer stack unchanged. The editor runtime's monitor introspection gained `Item` downcast arms (thumbnails, `vector_modify`), and Path Modify is an element-wise kernel whose unconnected content resolves to the empty `Item<Vector>` default. Old documents are covered by the generic stale-`List`-`TypeDefault` migration plus demand-driven variant selection: existing `List` wires keep selecting mapped variants, so nothing re-ranks under a saved document.
5. **Parameter ranking.** LANDED alongside stages 2-3: the math, string, vector-shape, value/color/text, raster, web-request, and context-reader families all take `Item`-ranked params, with bare stored `TaggedValue`s wrapping at resolution. Remaining bare data connectors are confined to infrastructure nodes (render pipeline, editor API, resource resolution) that stay bare by design, plus Path Modify's editor-injected modification.
6. **Generator parameter ranking (frame-from-params).** LANDED. `()`-primary generators take ranked params and frame over them via the mapped variant (no index stamp, so kernels need no context-extraction bounds), so `Circle(radius: List<f64>)` emits one shape per slot while `Item` params produce a one-frame `List` for document compatibility.
7. **Node family completion.** Substantially landed: Attach Attribute, the Option debug trio, Upload Texture, and Index Elements are deleted (each with a migration); the companion nodes all exist (Sum, Average, Minimum, Maximum, Any, All, Filter, Sort, Box Corners, Text to Vector Glyphs); Flatten Path is renamed Combine Paths; Extract Transform is element-wise (`Item<T> -> Item<DAffine2>`, with a `DVec2 -> Vector` field-adapter conversion restoring the Origins to Polyline body). Map and Map String stay as-is until the lazy-evaluation chapter by explicit ruling.
8. **Deferred beyond this branch (explicitly de-scoped).** Copy to Points and Repeat on Points stay, fully ranked and working; their deletion in favor of Transform broadcast waits until after the merge, and Repeat on Points may additionally wait on the eager-to-lazy refactor since it evaluates its content per element with a unique index/position context that Transform's zip does not yet supply. The assign/spread family is likewise post-merge work; Assign Colors keeps its whole-collection form (verified working in-app). (The Brush trace `Item<BrushTrace>` newtype is now landed.)
9. **Graphic rank untangling (deferred to the Graphic-lowering phase).** Everything about ranking `Graphic`-typed connectors stays in its current bare-`List` form for now, deliberately: Fill's paint connector (bare `List<Graphic>` broadcast today), the layer-conversion nodes (auto-conversion / Wrap Graphic / To Graphic), and the general "one paint that is a list vs. a list of paints" collision. These are all facets of the same untangling and belong with the future Graphic lowering, not this PR. The current behavior is livable and is in fact the *least surprising* to users today: in the present reality many graphics are lists and per-element zipping is not part of the mental model, so introducing paint-zipping before Graphic is lowered would surprise more than it helps.
10. **Later horizons.** Data-tree spines (rank >= 2 as data), demand-driven broadcast-as-re-evaluation in the adapters, frame-total exposure (which would let Assign Colors go element-wise), and possibly bare-`T` kernel sugar once the semantics are settled.

## Remaining before merge

- Frontend `npm run check`, intensive in-app testing, rebase onto master, and the PR.

(Fill's paint connector and the layer-conversion node design were moved to the Graphic-lowering deferral above, item 9.)

### GPU shader kernels (`shader_node(PerPixelAdjust)`)

Landed. The raster adjustment/blending kernels are ordinary `Item<T>` element-wise nodes, and their per-pixel logic is shared with the GPU by re-emitting the kernel body verbatim rather than hand-writing a second function or extracting a closure. The CPU compilation sees `core_types::list::Item` (real, attribute-carrying); the SPIR-V compilation sees `no_std_types::list::ShaderItem`, imported `as Item`, a `#[repr(transparent)]` stand-in exposing only element access, so every `Item<T>` wrapper and `.element()` call resolves to a zero-cost identity. It is named distinctly from the canonical `Item` so a codebase search for the real type is unambiguous. A body that touches attribute APIs (meaningless per-pixel) fails the shader build loudly instead of misbehaving.

`PerPixelAdjust` codegen peels `Item` off ranked uniform parameters so the `repr(C)` uniform buffer stays bare, wraps the fetched texel and uniform values at the fragment entry point, and unwraps the returned item. No per-node annotation is needed; plain `shader_node(PerPixelAdjust)` continues to work. The `Adjust`/`Blend` per-element seams live on the element types (`Color`, `Raster<CPU>`, `Gradient`) rather than on `List`.
