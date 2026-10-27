# Summary

Replace the graph compilation pipeline (preprocess, flatten, nullification, stable IDs, typing, borrow tree construction) with a pipeline built on shared definitions, interned types, and a single post-order lowering pass that emits a topologically ordered IR without ever sorting.
Shared definitions are also a user-facing feature: a subgraph defined once and instantiated many times, where editing the definition changes every instance.

# Motivation

## Performance

Cold compilation of the demo artwork, measured on migrated documents through the editor's own load path (median of 20, release build):

| document | proto nodes | preprocess | compile | executor construction | total |
|---|---|---|---|---|---|
| red-dress | 4172 | 0.34 ms | 6.74 ms | 5.64 ms | 12.7 ms |
| isometric-fountain | 3232 | 0.27 ms | 4.48 ms | 4.58 ms | 9.3 ms |
| valley-of-spires | 2890 | 0.25 ms | 3.91 ms | 4.06 ms | 8.2 ms |
| painted-dreams | 1857 | 0.17 ms | 2.59 ms | 2.72 ms | 5.5 ms |
| changing-seasons | 632 | 0.05 ms | 0.77 ms | 1.05 ms | 1.9 ms |

That is roughly 3 µs per proto node, and every edit pays it because there is no incremental compilation.

A perf profile (DWARF call graphs, 300 iterations over all demos) attributes the time as follows, as shares of the whole run:

- `DynamicExecutor::new`: 43%, of which `TypingContext::infer` 27% (scanning registry impls per node, collecting `NodeIOTypes`, cloning and dropping `Type`, `valid_type`) and `BorrowTree::push_node` 15% (of which `update_source_map` 8.6%, a `HashMap<Box<[NodeId]>, _>` of document paths).
- `Compiler::compile`: 40%, of which `flatten_with_fns` 19% (cloning `DocumentNode`s and their input `Vec`s, `HashMap` inserts and rehashes), `reorder_ids` 11% (two full topological sorts with ID remapping), `generate_stable_node_ids` 3.7% (SipHash over value contents, including vector segment data), `into_proto_networks` 3.4%.
- Preprocessing: 3%.
- Across all phases, malloc/free and memmove account for about 40% of self time.

The dominant cost is copying and allocating, not computation.
Incremental fixes to the current pipeline (FxHash, one topological sort, fewer clones) are expected to give a small constant factor; reaching the target below needs a different representation.

## Duplication

| document | document nodes | after preprocessing | proto nodes | unique stable IDs | distinct identifiers |
|---|---|---|---|---|---|
| red-dress | 1228 | 4173 | 4172 | 2413 | 42 |
| isometric-fountain | 985 | 3278 | 3232 | 2099 | 46 |
| painted-dreams | 602 | 1990 | 1857 | 1205 | 53 |

The preprocessor wraps every node macro node in a generated network (one input adapter per field), which flattening then clones per use: a 3.4x expansion.
42% of the resulting proto nodes are duplicates that are only removed after being built, and the whole document uses about 45 distinct node identifiers.
Most typing and construction work repeats a small set of signatures.

## Semantics

Users need reusable subgraphs: edit a definition once and have every instance follow.
The `.gdd` storage format already supports this structurally (`Implementation::Network(NetworkId)` can be referenced by several nodes), but the runtime inlines every nested network, so `from_runtime` mints a fresh network per traversal and the sharing is lost.

# Goals

- Cold compile of red-dress, through executor construction, in under 0.5 ms (about 100 ns per proto node).
- Compile work proportional to distinct definitions and signatures, not to instances.
- No topological sort and no ID remapping pass.
- Type checking that supports context-injected types and integrates coercion and rank promotion, replacing post hoc promotions.
- A structure that admits incremental compilation later without redesign.
- A backend-neutral compiled IR, the successor of `ProtoNetwork`, consumed either by the interpreted executor or by a Rust codegen backend that emits a compilable source file.

# Guide-level explanation

## Definitions and instances

A *definition* is either a network (nodes, exports, typed imports) or a proto node identifier.
A *node* in a network is an *instance*: a definition reference plus input bindings.

```rs
pub struct DefinitionId(u32);

pub enum Definition {
	Network(NetworkDefinition),
	Proto(ProtoNodeIdentifier),
}

pub struct Instance {
	pub definition: DefinitionId,
	pub inputs: Vec<NodeInput>,
	// visible, call_argument, ...
}
```

The document owns a definitions table; the root network is one definition.
`DocumentNodeImplementation::Network(NodeNetwork)` becomes `Network(DefinitionId)`, mirroring `.gdd`'s `Implementation::Network(NetworkId)` so the storage conversion is one to one.

Editing a node inside a shared definition changes every instance.
Editing only one instance requires an explicit *make unique* operation, which copies the definition and repoints that instance.

## What compilation does

Compilation is two deterministic passes over the instance tree, with no backtracking:

1. **Context pass (top-down, from the output toward sources).**
   Computes each node's *context signature*: which context features are injected above it and with which types (for example the vararg item type injected by Write Attribute's `value` lambda).
   This is what lets a generic "read" node resolve its output type from an injection performed by a node called earlier in the call chain.
2. **Lowering pass (bottom-up, post-order).**
   For each node, with its inputs already lowered: select the implementation and any input coercions, compute its context features and nullification, hash-cons it, and append it to the IR.

Lowering is memoized per `(definition, input signatures, context signature)`, so a definition instantiated a hundred times with the same types is lowered once and referenced a hundred times.

# Reference-level explanation

## Type interning

`Type` is a recursive enum of boxes; cloning and comparing it is the largest single typing cost.
All types are interned into a process-wide `TypeInterner`:

```rs
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeId(u32);
```

Structural equality becomes integer equality, and node signatures (`call_argument`, inputs, output) are interned slices of `TypeId`.
The registry is indexed per identifier by input signature, so implementation lookup is a hash of a few integers instead of a scan over all impls with `valid_type` per candidate.
Generic resolution (`Generic("T")`) substitutes over interned types and caches the substitution result.

## Implementation selection and coercion

The compiler names no data types.
Coercion is driven by two inputs registered next to the types themselves, the same way nodes register today:

- **Conversions** are ordinary conversion node implementations (`from → to`); registering one makes the edge available.
- **A purity relation** is a partial order on data types, declared as "`A` is purer than `B`" edges (for example `Color` purer than `Vector`, `Vector` purer than `Raster<CPU>`).
  The registry takes the transitive closure once at startup and stores it as a bit matrix over interned `TypeId`s.

The cost of a conversion `A → B` is derived from the relation, never declared per conversion:

| relation of `A` to `B` | conversion kind |
|---|---|
| `A` = `B` | identity, no conversion |
| `A` purer than `B` | purity-losing, applied automatically |
| `B` purer than `A` | purity-gaining (lossy), governed by policy |
| incomparable | not applied automatically |

For one input:

1. If an implementation accepts the incoming type, no conversion is applied.
2. Otherwise at most one registered conversion is applied per edge (no transitive chains).
3. Among the reachable accepted types, the maximal element under the purity order wins (for example `Color` into `{Vector, Raster}` becomes `Vector`); incomparable maxima are an ambiguity error.

Whether purity-gaining conversions may be applied automatically is a policy deliberately left open; initially they are not registered as automatic.
For nodes with several generic inputs, the implementation is chosen jointly by the lexicographic sum of per-input conversion kinds.
Adding a data type means registering its conversions and its purity edges; the compiler does not change.
Rank promotion (`Item<T>` to `List<T>`) is a structural coercion over the `Item`/`List` type constructors, ranked as identity-preserving, which replaces the `promotions` map and the retry path in `TypingContext`.

Selection results are memoized by `(identifier, input TypeIds, context signature)`; with about 45 distinct identifiers per document the table stays tiny.
Adapters are emitted as ordinary IR nodes only on edges that need them, which replaces the preprocessor's blanket per-field wrapping.

## The IR

```rs
pub struct CompiledGraph {
	/// Topologically ordered: every node's inputs have lower indices.
	pub nodes: Vec<IrNode>,
	/// Flat input index storage, sliced by `IrNode::inputs`.
	pub inputs: Vec<u32>,
	pub values: Vec<MemoHash<TaggedValue>>,
}

pub struct IrNode {
	/// The selected implementation: the proto node identifier plus its resolved monomorphic signature.
	pub implementation: ImplementationId,
	pub inputs: Range<u32>,
	pub signature: SignatureId,
	pub features: ContextFeatures,
	pub content_hash: u64,
	pub origin: OriginId,
}
```

The IR names implementations by identifier and monomorphic signature, never by an executor's constructor function, so it stays backend-neutral.
The interpreted executor maps each `ImplementationId` to a registry constructor; a Rust codegen backend maps it to the node's Rust path with its type arguments and emits the graph as a source file, using the IR's order as the statement order.

Topological order is a property of construction: a node is appended only after all of its inputs (post-order), so no sort or remap is needed.
Cycles are detected by the same DFS's visiting state.
Hash-consing uses `content_hash = hash(implementation, input content hashes, value hash)`, which replaces `generate_stable_node_ids` and removes duplicates before they are built.
Value hashes come from `MemoHash`'s cached hash instead of rehashing value contents.

`origin` indexes a table of document locations that is materialized only when something asks for it (errors, the Data panel, monitor lookup), replacing the per-node `Box<[NodeId]>` path map that executor construction currently maintains.

## Context features and nullification

Context features are already a bottom-up union, and the current algorithm (`find_context_dependencies`) decides where to insert nullification at the parent from its branches' feature sets.
Both therefore fold into the lowering pass, together with the footprint invariance and other compile-time checks.

## Executor construction

The borrow tree becomes a `Vec` of node containers indexed by IR index, with inputs resolved by index instead of `HashMap<NodeId, _>` lookups.
Across recompiles, containers are reused by content hash, as stable node IDs allow today.

## Allocation budget

- One flat buffer per IR table, reused across compiles; no per-node `Vec`.
- No cloning of document nodes: lowering reads the definitions table by reference.
- FxHash wherever hashing remains.
- The source map is materialized on demand only.

## Incremental compilation (future)

The lowering memo key, `(definition content hash, input signatures, context signature)`, is also a cache key across compiles: a definition untouched by an edit, used under the same signatures, keeps its lowered fragment.
`NodeNetworkInterface` change events mark the edited definitions dirty.
This works because both passes are deterministic and each node's result depends only on its upstream cone plus its context signature.

## Storage

`.gdd` already stores a networks table, so the runtime/storage conversion becomes structural and `from_runtime` stops duplicating networks.
Legacy documents contain inlined copies of nested networks.
These must not be merged into shared definitions automatically, since that would silently change edit semantics.
Only compiler-generated structure (former preprocessor wrappers, adapters) is shared implicitly.

# Migration plan

1. Benchmark harness: the editor-hosted timing harness over migrated demos becomes a committed benchmark, and the interpreted-executor benchmarks (currently broken on raw legacy files) load migrated fixtures.
2. Type interner, usable by both the old and new pipelines.
3. Definitions table in the runtime `NodeNetwork`, `NodeNetworkInterface`, and the storage conversion.
4. New pipeline next to the old one, with a differential test that evaluates every demo through both and compares outputs.
5. Coercion and rank promotion in the lowering pass; remove the preprocessor's wrapper generation.
6. Switch over and delete the old pipeline.

# Drawbacks

- A large change touching the editor, storage conversion, compiler, and executor at once.
- Shared definitions add user-facing concepts (shared vs unique, make unique) that need UX work.
- Two pipelines coexist during the transition.

# Rationale and alternatives

- **Quick wins in the current pipeline** (FxHash, a single topological sort, interned types only): lower risk, but the representation still clones every instance and builds duplicates before removing them, which caps the speedup at a small constant factor.
- **A query system (Salsa-style)**: more general incrementality, but it adds a framework before the basic pipeline is fast; the memo key above can migrate to queries later.
- **Set-of-types propagation with backward commitment**: more expressive resolution, but the chosen implementation would no longer be a function of a node's upstream cone, which conflicts with cone-local incremental compilation. The context pass covers the motivating case (types injected by callers) deterministically.

# Unresolved questions

- **Per-instance inputs under sharing**: `NodeInput::Reflection` (a node's own location), monitor nodes for layer thumbnails, and `skip_deduplication` are inherently per instance. Proposal: lift them into implicit definition parameters supplied by each instance, so the definition body stays shared.
- **Typed imports**: definitions need declared import types to be lowered once per signature; today import types live on each `NodeInput::Import`.
- **Context signature representation**: which injected facts it carries (feature set, vararg item types, later attribute schemas) and how it is interned.
- **Lossy automatic conversions**: policy deferred; initially not applied automatically.
- **Purity declarations**: where the edges are declared (on the type, with a derive or attribute, or as standalone registrations in the crate defining the conversion) and how an inconsistent declaration (a cycle) is reported.
- **`Extract` implementations and scope injections** under shared definitions.
- **Make unique** semantics and UX in `NodeNetworkInterface`, including undo and collaboration deltas.

# Future possibilities

- Attribute schemas carried in list types, so reads downstream of a write on the same list are typed forward (requires constant attribute names).
- Persistent cross-compile fragment cache, keyed by the lowering memo key.
- JIT or precompiled emission of hot subgraphs from the IR.
