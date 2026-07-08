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

1. **Batch migration (mechanical, no new machinery).** LANDED. All audit-classified element-wise nodes are converted to `Item<T>` kernels, ending with Combine Channels (ranked channels under its `()` primary), Map Points, and the Upload Texture deletion. Three conversions are deliberately deferred by dependency, not omission: Fill's paint connector (needs zip-aware document wiring to replace its whole-`List` fill collapse), Extract Transform (currently the element-0 demoter that Origins to Polyline's Map body depends on), and To Graphic (pending the auto-conversion subsumption decision). Path Modify converts at stage 4, where its empty-input synthesis and the layer coercion path become rank-0 capable.
2. **Rank-aware resolution and promote adapters (compiler).** LANDED. The typing context retries failed resolutions with per-input promotion adapters (`WrapItemNode` bare -> `Item`, `ItemToListNode` `Item` -> singleton `List`, `WrapListNode` bare -> `List` at double cost, `UnwrapItemNode` `Item` -> bare for legacy connectors), picking the variant with the cheapest total promotion cost and leaving ties ambiguous. Promotions are recorded per node and applied by the executor at construction time as argument wrappers, so the graph structure itself stays adapter-free. The preprocessor's generated wrapper networks route ranked fields through `FieldAdapterNode` (wrap or sanctioned element conversion) instead of `IntoNode`/`ConvertNode`.
3. **Multi-connector zip.** LANDED. The macro's mapped variant zips every ranked connector by frame slot (longest-list, last-element repeats), broadcasts bare parameters by clone, and stamps the slot index onto the context for lazy connectors, which re-evaluate per slot; this is what delivers `Item<Vector>` content x `List<DVec2>` translations -> `List<Vector>` through Transform, and it retired the hand-written Transform-zip, Area, and Centroid companions. Expander kernels flat-map under the frame per the rank-2 force-flatten rule. The lazy-primary list-content variant complements it by framing over the content length with a precomputed per-slot stub. Attribute precedence needs no merge step: the kernel writes through its primary item's attributes.
4. **Generator flip + document migration.** NEXT. Generators emit `Item<T>`; document upgrade inserts promotions where old documents expect lists; frontend displays `Item<T>` as `T`. Rank-0 wires begin flowing through real documents. Known editor-side prerequisites: the layer coercion trio (`to_graphic`, `wrap_graphic`, `extend`) needs rank-0 forms, the monitor introspection in the editor runtime needs `Item` downcast arms, and Path Modify converts here (element-wise kernel, `Item`-typed unconnected default, `vector_modify` introspection arm).
5. **Parameter ranking.** Substantially landed alongside stages 2-3: the math, string, vector-shape, value/color/text, raster, web-request, and context-reader families all take `Item`-ranked params, with bare stored `TaggedValue`s wrapping at resolution. Remaining bare data connectors are confined to infrastructure nodes (render pipeline, editor API) that stay bare by design.
6. **Generator parameter ranking (frame-from-params).** LANDED. `()`-primary generators take ranked params and frame over them via the mapped variant (no index stamp, so kernels need no context-extraction bounds), so `Circle(radius: List<f64>)` emits one shape per slot while `Item` params produce a one-frame `List` for document compatibility.
7. **Node family completion.** Partially landed: Attach Attribute and the Option debug trio are deleted; the companion nodes all exist (Sum, Average, Minimum, Maximum, Any, All, Filter, Sort, Box Corners, Text to Vector Glyphs). Map and Map String stay as-is until the lazy-evaluation chapter by explicit ruling. Remaining: delete Copy to Points and Repeat on Points (unblocked now that Transform broadcasts), Extract Element, and the As-type trio; land the assign/spread family.
8. **Later horizons.** Data-tree spines (rank >= 2 as data), demand-driven broadcast-as-re-evaluation in the adapters, and possibly bare-`T` kernel sugar once the semantics are settled.

### GPU shader kernels (`shader_node(PerPixelAdjust)`)

Landed. The raster adjustment/blending kernels are ordinary `Item<T>` element-wise nodes, and their per-pixel logic is shared with the GPU by re-emitting the kernel body verbatim rather than hand-writing a second function or extracting a closure. The CPU compilation sees `core_types::list::Item` (real, attribute-carrying); the SPIR-V compilation sees `no_std_types::list::ShaderItem`, imported `as Item`, a `#[repr(transparent)]` stand-in exposing only element access, so every `Item<T>` wrapper and `.element()` call resolves to a zero-cost identity. It is named distinctly from the canonical `Item` so a codebase search for the real type is unambiguous. A body that touches attribute APIs (meaningless per-pixel) fails the shader build loudly instead of misbehaving.

`PerPixelAdjust` codegen peels `Item` off ranked uniform parameters so the `repr(C)` uniform buffer stays bare, wraps the fetched texel and uniform values at the fragment entry point, and unwraps the returned item. No per-node annotation is needed; plain `shader_node(PerPixelAdjust)` continues to work. The `Adjust`/`Blend` per-element seams live on the element types (`Color`, `Raster<CPU>`, `Gradient`) rather than on `List`.
