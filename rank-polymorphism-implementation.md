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

1. **Batch migration (mechanical, no new machinery).** Convert the remaining audit-classified element-wise nodes to `Item<T>` kernels. Each conversion is wire-compatible; behavior deviations are only those already approved in the audit.
2. **Rank-aware resolution and promote adapters (compiler).** Teach the preprocessor/typing context to insert promotion adapters (bare value -> `Item<V>`; `Item<V>` -> broadcast `List<V>`), following the existing `IntoNode` insertion precedent. This unlocks mixed-rank wiring and is the prerequisite for zip.
3. **Multi-connector zip.** Extend the mapped variant to zip all ranked connectors sharing the frame (longest-list, last-element repeats; attribute merge primary-first), replacing the single-mapped-input limitation. This delivers `Item<Vector>` content x `List<DVec2>` translations -> `List<Vector>` through Transform.
4. **Generator flip + document migration.** Generators emit `Item<T>`; document upgrade inserts promotions where old documents expect lists; frontend displays `Item<T>` as `T`. Rank-0 wires begin flowing through real documents.
5. **Parameter ranking.** Scalar connectors move to `Item<f64>`-style wires with TaggedValue promotion in the preprocessor, making every data connector frameable.
6. **Generator parameter ranking (frame-from-params).** `()`-primary generators currently force bare params because validation requires an `Item` primary before params may rank; that is transitional, not a rule. Add a unit-primary mapped variant whose frame comes from the ranked params (the same machinery the Transform lazy-primary variant proved), so `Circle(radius: List<f64>) -> List<Vector>` emits one shape per slot. Until this lands, the signatures report exempts generator params from flagging.
7. **Node family completion.** Delete Copy to Points, Repeat on Points, Map, Map String, Attach Attribute, Extract Element, the As-type trio, and the Option debug trio; land the assign/spread family; add the companion nodes (Sum, Average, Minimum, Maximum, Any, All, Filter, Sort, Corners, Separate Glyphs).
8. **Later horizons.** Data-tree spines (rank >= 2 as data), demand-driven broadcast-as-re-evaluation in the adapters, and possibly bare-`T` kernel sugar once the semantics are settled.

### GPU shader kernels (`shader_node(PerPixelAdjust)`)

Landed. The raster adjustment/blending kernels are ordinary `Item<T>` element-wise nodes, and their per-pixel logic is shared with the GPU by re-emitting the kernel body verbatim rather than hand-writing a second function or extracting a closure. The CPU compilation sees `core_types::list::Item` (real, attribute-carrying); the SPIR-V compilation sees `no_std_types::list::ShaderItem`, imported `as Item`, a `#[repr(transparent)]` stand-in exposing only element access, so every `Item<T>` wrapper and `.element()` call resolves to a zero-cost identity. It is named distinctly from the canonical `Item` so a codebase search for the real type is unambiguous. A body that touches attribute APIs (meaningless per-pixel) fails the shader build loudly instead of misbehaving.

`PerPixelAdjust` codegen peels `Item` off ranked uniform parameters so the `repr(C)` uniform buffer stays bare, wraps the fetched texel and uniform values at the fragment entry point, and unwraps the returned item. No per-node annotation is needed; plain `shader_node(PerPixelAdjust)` continues to work. The `Adjust`/`Blend` per-element seams live on the element types (`Color`, `Raster<CPU>`, `Gradient`) rather than on `List`.
