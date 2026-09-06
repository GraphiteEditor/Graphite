# The Graphite Node Graph

Graphite is a node based image editor. Everything it renders comes from evaluating a graph of nodes, and this directory holds that system: the types nodes are built on, the node library, the macro that turns a plain Rust function into a node, and the compiler and executor that turn a saved document into a running graph.

This is an orientation document, not a specification. It says what lives where and what the vocabulary means. The code is the ground truth, and the module level `//!` headers under `libraries/core-types/src/` carry the real design detail.

## Crate map

Foundations, in `libraries/`:

| Crate | Directory | What it is |
| --- | --- | --- |
| `core-types` | `libraries/core-types` | The `Node` trait, records, attributes, the arena, contexts, and the node registry. Everything else sits on it. |
| `no-std-types` | `libraries/no-std-types` | `no_std` primitives shared with the GPU shader crates: color, blending, choice types. |
| `graphene-hash` | `libraries/graphene-hash` | `CacheHash`, which hashes floats by bit pattern so cache keys stay stable. |
| `graphic-types` | `libraries/graphic-types` | Document element types: `Graphic`, `Vector`, `Artboard`, and their attribute markers. |
| `raster-types` | `libraries/raster-types` | `Image`, `Raster<CPU>` and `Raster<GPU>`, `Texture`, and the pixel traits. |
| `vector-types` | `libraries/vector-types` | Vector geometry: `Vector`, subpaths, point and segment domains, gradients, styles. |
| `rendering` | `libraries/rendering` | The `Render` trait and the SVG and Vello backends. |
| `graphene-application-io` | `libraries/application-io` | Traits for reaching the host: GPU access, resource loading, `EditorApi`. |
| `graphene-resource` | `libraries/resources` | Content addressed binary blobs and their async loading. |
| `graphene-canvas-utils` | `libraries/canvas-utils` | HTML canvas helpers, and nothing at all off wasm. |
| `wgpu-executor` | `libraries/wgpu-executor` | The GPU backend: wgpu context, pipeline and texture caches, the Vello renderer. |

The node library, in `nodes/`:

| Crate | Directory | What it is |
| --- | --- | --- |
| `graphene-core` | `nodes/gcore` | Core nodes: arithmetic, context modification, memoization, animation, and the pilot record nodes. |
| `graphene-std` | `nodes/gstd` | The standard library. Re-exports every other node crate and adds the render pipeline, text, and platform IO nodes. |
| `blending-nodes` | `nodes/blending` | Blend mode, opacity, and fill. |
| `brush-nodes` | `nodes/brush` | Brush strokes, stamping, and brush rendering. |
| `graphic-nodes` | `nodes/graphic` | Grouping and artboard construction over `Graphic`. |
| `math-nodes` | `nodes/math` | The expression parser node plus arithmetic, random, and vector math. |
| `path-bool-nodes` | `nodes/path-bool` | Boolean path operations on vector geometry. |
| `raster-nodes` | `nodes/raster` | Color adjustments, blending, and filters. Written `no_std` capable so the kernels double as GPU shaders. |
| `repeat-nodes` | `nodes/repeat` | Grid, linear, and circular repeats. |
| `text-nodes` | `nodes/text` | Font loading, glyph shaping, text to path, and string operations. |
| `transform-nodes` | `nodes/transform` | Translation, rotation, scale, skew, and footprint aware transforms. |
| `vector-nodes` | `nodes/vector` | Shape generators, path operations, and vector modification. |

Compiler and runtime:

| Crate | Directory | What it is |
| --- | --- | --- |
| `graph-craft` | `graph-craft` | The document graph and its lowering: `NodeNetwork` and `DocumentNode` down to `ProtoNetwork`, plus type inference. |
| `preprocessor` | `preprocessor` | The pass over a freshly loaded document: expands proto nodes into their definitions, injects scopes, resolves resource inputs. |
| `interpreted-executor` | `interpreted-executor` | The dynamic executor. Instantiates a `ProtoNetwork` into a tree of boxed nodes and evaluates it. |
| `node-macro` | `node-macro` | `#[node]` and friends. Generates the node struct, its `Node` impl, the registry entries, and the editor metadata. |
| `graphene-cli` | `graphene-cli` | Headless CLI: load a document, compile it, run it, export the result. |

## Core concepts

### Nodes

`Node` lives in `libraries/core-types/src/node.rs` and has exactly one required method, `serve`. A node serves its output record through a frame claim its caller hands it, and returns a proof that only the claim's own closing methods can mint, so a served record is of the claimed layout by construction rather than by convention. Each node claims its frame out of the free space its caller left, so the frame space divides without bookkeeping.

Everything else on the trait has a default. `extent_at` and `extent` report how many items the node produces at a nesting level, `layout` reports the record layout, and `eval_batch` serves a whole range at once. The default `eval_batch` advertises no support and drivers fall back to per lane serves with copy out, so an override exists to beat that loop and never for correctness.

Kernels written with the macro do not see `serve`. They receive typed inputs and call `eval` on them, and the generated code handles the claim, the protocol, and status folding.

### Records

A record is what flows between nodes: the element at offset 0 plus one field per attribute written upstream. Its `Layout` is computed at wiring time from the upstream write set and is never serialized. Records of inline layouts live in the `RecordValue` itself, and larger ones live as per lane views on the evaluation's `Frames`, which the root owns and every node claims its own frame out of. Only generated and wiring code touches offsets, so a safe kernel cannot misalign a field.

`libraries/core-types/src/record/` is split by abstraction level, and each module's `//!` header states its job:

| Module | Concern |
| --- | --- |
| `layout` | Wiring time shape facts: the layout a record takes and the writes it folds from. |
| `access` | Raw typed access to a record at a wiring proven layout. |
| `frames` | The evaluation's record frame storage. |
| `serve` | The serving protocol: a node's own frame claim and the proof it closes with. |
| `input` | Consumer side bindings onto a record input, and the drivers they run. |
| `route` | Producer side routing: a source's translation into the union layout. |
| `promote` | Transient to persistent promotion of records and their parked payloads. |
| `owned` | The owned crossing: deep copies that outlive the evaluation their content borrowed. |
| `run` | Runs and groups: many records of one layout, resident or owned. |
| `testkit` | Law test scaffolding over the record tier. |

Two words are used precisely throughout. An input is the consumer side of a connection, and a source is the producer side. Neither is called a wire; that word belongs to the editor UI.

### Attributes

An attribute is a named, typed channel that rides along with an element. A marker declares the name once, fixing its value type and its name specific default, and a census collects every declaration so name resolution, defaults, and diagnostics all happen at graph compile time. One name belongs to one marker, so a name can never mean two types. Declare markers with `core_types::attribute!`:

```rust
core_types::attribute! {
	/// The measured length of an element.
	pub Length("length"): f64;
	/// A label parked in the arena by whoever writes it.
	pub Label("label"): &str;
}
```

Values are `Copy` and pack directly into record fields. Anything with drop glue rides the arena instead: the marker declares a reference value such as `&str`, the writing kernel parks the payload in the arena, and the record field carries a reference good for the evaluation.

In a kernel, `Attr<A>` as a parameter is a read, yielding the declared default where nothing upstream wrote it. `Attr<A>` in the return tuple is a write, and the same marker on both sides is a modify. `OwnedAttr<A>` carries a value across the evaluation boundary, and `RemoveAttr<A>` subtracts the attribute from the layout. See `libraries/core-types/src/attribute.rs`, and `nodes/gcore/src/record.rs` for worked examples of every form.

### The arena

Two regions, both in `libraries/core-types/src/arena.rs`. The transient arena is reset at the top of every evaluation, so anything parked in it lives exactly as long as the evaluation that parked it. The persistent region backs promoted memo levels; it is flushed whole between evaluations and never during one, so no flush can land while a promoted value is still readable.

`Arena::move_park` is the promotion. It copies a payload's header into the receiving region, hands over the drop obligation, and tombstones the source entry in place. The heap the payload owns is neither copied nor freed, since ownership travels with the obligation, and a payload two records share moves once. Region sizes and the flush policy live in `interpreted-executor/src/dynamic_executor.rs`.

### Serving

Evaluating a graph is a walk of `serve` calls down from the root. The root claims the frame buffer, each node claims its own frame out of what its caller left, and status rides the `GPoll` return: `Final`, `Partial` for a result that is correct but incomplete, `Pending` for work not yet ready, `Error`, and `Fallback` for a usable value paired with the error that degraded it. `StatusCell` folds each input's status into the serving node's own, which is what lets a kernel be written as though its inputs simply returned values.

## Adding a node

Write a function and put `#[node_macro::node]` on it. The macro generates the node struct, the `Node` implementation, the registry entries, and the metadata the editor builds its catalog and properties panel from.

```rust
use core_types::Ctx;
use core_types::attribute::{Attr, Opacity};

/// Scales the opacity attribute of every element passing through.
#[node_macro::node(category("Raster: Adjustments"))]
fn multiply_opacity(
	_: impl Ctx,
	/// The element and its current opacity.
	(element, opacity): (f64, Attr<Opacity>),
	/// The factor to scale the opacity by.
	#[default(1.)]
	#[range]
	#[soft(0..2)]
	factor: f64,
) -> (f64, Attr<Opacity>) {
	(element, Attr(*opacity * factor))
}
```

Reading that back: the first parameter is the evaluation context. A tuple parameter is a record input, binding the element alongside the attributes this kernel reads, while a plain parameter is an ordinary value input the user can set. The return tuple is the write set, and returning `Attr<Opacity>` after reading it makes this a modify rather than a fresh write.

Doc comments are not decoration. The one on the function becomes the node's description in the editor's catalog, and the one on each parameter becomes that input's description.

Options on the invocation are `category`, `name`, `path`, `properties` (naming a function in `editor/src/messages/portfolio/document/node_graph/node_properties.rs` for a custom panel), `extent` (naming a function that computes the node's extent, for nodes that change the item count), and `skip_impl`.

Per parameter there are `#[default(..)]`, `#[expose]`, `#[name(..)]`, `#[widget(..)]`, and `#[implementations(..)]`, which enumerates the concrete type rows a generic node registers. For numbers, `#[range]` renders a draggable slider, `#[soft(a..b)]` sets its suggested extent, and `#[hard(a..b)]` sets the enforced clamp. Either endpoint may be omitted and both are inclusive, so there is no `..=` form. Typed values may exceed the soft extent but are clamped to the hard bounds, so `#[soft]` only means anything together with `#[range]`. The macro checks these and will reject a `#[range]` missing an end, or a `#[soft]` bound equal to its `#[hard]` counterpart.

Registration is automatic. The macro emits a `ctor` constructor that inserts into `NODE_REGISTRY` and `NODE_METADATA` in `libraries/core-types/src/registry.rs` at process start, and `interpreted-executor/src/node_registry.rs` takes that table and adds a hand written set of conversion nodes to build the registry the compiler consumes. There is no separate definition table to edit; writing the function is the whole job. On wasm the `ctor` attribute is skipped and registration is driven explicitly instead.

## From a document to a result

A saved document arrives as a `NodeNetwork`. The preprocessor expands each proto node reference into the `DocumentNode` template built from its registry metadata, reconciling saved inputs against what the current definition expects, which is what lets old documents load against new node definitions. `NodeNetwork::flatten` then dissolves nested networks into one graph, `into_proto_networks` lowers it to a `ProtoNetwork`, and `TypingContext` resolves types against the registry. The executor pushes each `ProtoNode` into its `BorrowTree`, which keeps a node alive as long as anything references it, and `eval_root` resets the transient arena, sizes the frame buffer to the graph's need, and serves the output.

## Deeper reading

The design documentation for the record tier is the `//!` headers in the source. Read `libraries/core-types/src/record/mod.rs` first, then the submodule headers in the table above, then `node.rs`, `attribute.rs`, and `arena.rs`.

Two RFCs cover work adjacent to this system:

- [`rfcs/document-format.md`](rfcs/document-format.md), the `.gdd` on disk format and its CRDT delta model.
- [`rfcs/fine-grained-context-caching.md`](rfcs/fine-grained-context-caching.md), a compilation pass that nullifies unused parts of the context to avoid needless cache invalidation.

## Debugging

`log::debug!()` works inside a node body. For running a graph without the editor in the way, `graphene-cli` loads a document, compiles it, evaluates it, and can list every registered node.

If any of this is wrong or unclear, please ask in the Graphite Discord. We are happy to help.
