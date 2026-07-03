# Rank Polymorphism Implementation

Status and staging plan for the rank polymorphism refactor. The classification that drives this work is in `rank-polymorphism-node-audit.md`.

## Implemented (this branch)

### Element-wise kernels, declared structurally by `Item<T>`

A node whose primary input is typed `Item<T>` is an element-wise rank-0 kernel: it consumes one item per call, with full access to the item's element and attributes. No macro attribute is needed; the signature is the declaration. The macro generates two registry variants per implementations row under the same node identifier:

- **`Item<T>` variant** (struct `XNode`): evaluates the primary input as one `Item<T>` and calls the kernel once. Output wire: `Item<U>`. Dormant until rank-0 wires flow (generator flip stage), but registered and type-checked now.
- **`List<T>` variant** (struct `XNodeMapped`): evaluates the primary input as a `List<T>` and calls the kernel once per item, broadcasting all other parameters by clone. Output wire: `List<U>`. This exactly replaces today's hand-written per-row loops, so existing documents resolve to it with identical behavior.

The kernel owns the item wrapping on both sides: it receives `Item<T>`, may read or write attributes, and returns `Item<U>`. Generic element-wise nodes list bare element types in `#[implementations(...)]` (e.g. `Graphic, Vector, Raster<CPU>`); the macro derives both wire forms. Field metadata, `NodeInputDecleration` accessors, and default types keep the `List` wire form for document compatibility.

Rules enforced by validation: only the primary input may be `Item`-typed; an `Item<T>` primary requires an `Item<U>` return and bare-element implementations; an `Item<U>` return requires an `Item<T>` primary; incompatible with `shader_node` (GPU kernels need bare `repr(C)` values). Bare-typed primaries continue to mean today's plain wires, so unmigrated nodes are untouched.

Migrated so far: Bounding Box, Close Path, Blur, Median Filter, Blend Mode, Opacity, Clipping Mask, Reset Transform, Replace Transform. The attribute-manipulating nodes became one-line kernels (`content.set_attribute(ATTR_BLEND_MODE, blend_mode)`), which deleted the ~180 lines of `SetBlendMode`/`MultiplyAlpha`/`MultiplyFill`/`SetClip` trait impls in the blending crate and resolved its three "apply once to the list's parent" TODOs — the `Item` variant is precisely that path.

### Supporting changes

`Item<T>` implements `StaticType`, making it a legal type-erased wire type.

## Authoring guide

```rust
#[node_macro::node(category("Blending"))]
fn blend_mode<T>(
	_: impl Ctx,
	#[implementations(Graphic, Vector, Raster<CPU>, Color, GradientStops, String)]
	mut content: Item<T>,
	blend_mode: BlendMode,
) -> Item<T> {
	content.set_attribute(ATTR_BLEND_MODE, blend_mode);
	content
}
```

Kernels that ignore attributes simply pass them through by mutating the element in place (`content.element_mut()`) or via `into_parts`/`from_parts` when the element type changes. Bare-`T` kernel sugar (macro-owned attribute plumbing) is deliberately deferred until the system is proven; `Item<T>` everywhere is the one authoring style.

## Staged roadmap

1. **Batch migration (mechanical, no new machinery).** Convert the remaining audit-classified element-wise nodes to `Item<T>` kernels. Each conversion is wire-compatible; behavior deviations are only those already approved in the audit.
2. **Rank-aware resolution and promote adapters (compiler).** Teach the preprocessor/typing context to insert promotion adapters (bare value -> `Item<V>`; `Item<V>` -> broadcast `List<V>`), following the existing `IntoNode` insertion precedent. This unlocks mixed-rank wiring and is the prerequisite for zip.
3. **Multi-connector zip.** Extend the mapped variant to zip all ranked connectors sharing the frame (longest-list, last-element repeats; attribute merge primary-first), replacing the single-mapped-input limitation. This delivers `Item<Vector>` content x `List<DVec2>` translations -> `List<Vector>` through Transform.
4. **Generator flip + document migration.** Generators emit `Item<T>`; document upgrade inserts promotions where old documents expect lists; frontend displays `Item<T>` as `T`. Rank-0 wires begin flowing through real documents.
5. **Parameter ranking.** Scalar connectors move to `Item<f64>`-style wires with TaggedValue promotion in the preprocessor, making every data connector frameable.
6. **Node family completion.** Delete Copy to Points, Repeat on Points, Map, Map String, Attach Attribute, Extract Element, the As-type trio, and the Option debug trio; land the assign/spread family; add the companion nodes (Sum, Average, Minimum, Maximum, Any, All, Filter, Sort, Corners, Separate Glyphs).
7. **Later horizons.** Data-tree spines (rank >= 2 as data), demand-driven broadcast-as-re-evaluation in the adapters, GPU/`shader_node` integration with element-typed implementations, and possibly bare-`T` kernel sugar once the semantics are settled.
