# Runtime deltas: design context and follow-up plan

Working notes for the in-vivo runtime delta effort on this branch, recording what exists, the
motivation behind each decision, and the context needed to complete the remaining phases. The
decisions here were settled in design discussion between Keavon and TrueDoctor during July 2026.

## What exists on this branch

- `graph_craft::runtime_delta::RuntimeDelta`: the structural delta enum. Payload-carrying,
  constructed at mutation time, plain runtime types, no serialization. Variants: `AddNode`,
  `ReplaceNode` (carrying `Box<DocumentNode>`), `RemoveNode` (address-only), `SetInput`,
  `SetExport`, `SetVisibility`.
- `editor::...::network_interface::editor_delta`: the editor-side wrapper `EditorDelta
  { Graph(RuntimeDelta), NodeMetadata { .. } }` and `construct_batch`, which turns one gesture's
  deltas into storage `RegistryDelta` ops.
- Parity tests (`editor_delta_tests`) proving the constructed ops equal the ops a whole-document
  diff would stage: exactly (op-for-op) for slot, export, visibility, and metadata changes;
  by stored-state equivalence for structural add/remove batches.
- Substrate reused from the earlier marker-based experiment: `ScopedConversion` (per-entity
  conversion through the canonical `from_runtime` encoders), `PathResolver` (path-hash ID
  derivation without conversion), `Session::stage_computed_ops`, `GddV1::stage_runtime_deltas`.

Not yet wired: mutation sites do not emit deltas, staging does not consume them, and the compiler
still receives whole-network clones.

## Decisions and their motivations

**In-vivo payload deltas, not dirty-set markers.** An earlier experiment (branches
`runtime-deltas-1-record` / `runtime-deltas-3-flip`, kept for reference) recorded scope markers and
re-derived deltas at staging by scoped reconversion. Rejected because the perf case did not hold
holistically: the editor stages after every mutation for the compiler anyway, so the data is in
hand at mutation time regardless, and payload capture costs nothing extra. Payloads also make the
delta a bidirectional currency (storage, compiler, and later undo/collab all speak it), which
markers structurally cannot.

**Graph-craft structural enum plus an editor wrapper.** The compiler must consume deltas without
editor dependencies, and a separate compiler-only delta type (a third set) was explicitly ruled
out. `SetInput`/`SetExport` are separate variants so the editor's `InputConnector` type does not
leak into graph-craft.

**Wholesale metadata copies, diffed by storage.** Rather than per-field metadata variants, one
`NodeMetadata` delta carries a copy of the node's persistent metadata (cheap to clone). Storage
diffs it against the working registry so minimal attribute ops fall out; the compiler throws it
away. This also absorbs ambiguity: the delta does not need to know which fields changed.

**`SetVisibility` is structural.** `visible` is a `DocumentNode` field the compiler consumes for
rendering. The rare other scalar setters (`call_argument`, `context_features`) go through
`ReplaceNode` instead of dedicated variants.

**Batch-scoped construction.** Removal closures and resource liveness are properties of the whole
gesture, not one delta: deleting a group with children removes several nodes whose shared
declaration only becomes unreferenced once all of them are gone. `construct_batch` is therefore
the construction unit, matching how mutations emit (compound operations produce several deltas)
and how staging consumes.

**Arc-backed input values (agreed, not yet implemented).** `NodeInput::Value`'s `TaggedValue`
storage should become `Arc`-backed (inside the existing `MemoHash`), mutating via `Arc::make_mut`.
Delta capture, compiler updates, and undo snapshots then share by pointer bump instead of cloning;
copy-on-write fires only when an old version is genuinely still held, which is exactly when a copy
is semantically required. This is a prerequisite for cheap drag/paint deltas and should land in
graph-craft before the mutation sites are wired.

**Accumulation and desync carry over from the earlier design.** A pending buffer on the interface
accumulates delta batches with same-target coalescing (a drag keeps first-to-last one `SetInput`).
Anything that cannot itemize its changes (document open, snapshot install, raw network access,
storage mount) desyncs the buffer, forcing one whole-document diff before delta staging resumes.
The `verify_journal_projection` soak pattern from the earlier branches (constructed ops compared
against the whole-document diff on every staging under the `validate_storage_round_trip`
preference) is the validation harness to reuse; it caught real bugs twice there.

## Remaining phases to land forward deltas

1. Arc-backed values in graph-craft.
2. The pending buffer on `NodeNetworkInterface` (accumulation, coalescing, desync states).
3. Site wiring: roughly thirty mutation sites emit their delta batches, capturing state after
   mutating. The site catalog from the earlier experiment maps every `transaction_modified` call
   site to what it touches.
4. Staging: `construct_batch` into `Session::stage_computed_ops` via `GddV1::stage_runtime_deltas`,
   with the whole-document diff as the desync fallback and the parity soak in validate mode.
5. Compiler consumption: replace the per-update whole-network clone in `GraphUpdate` with the
   `RuntimeDelta` stream reconciling a mirror on the runtime thread, using apply-reported change
   as the invalidation signal.

## Follow-up PR: storage-driven undo (decided: debug-compare rollout)

Direction: undo/redo stop installing interface snapshots and instead replay deltas returned by the
storage layer, whose retirement machinery already precomputes every delta's reverse and applies it
to the working registry on cursor moves.

Shape agreed in discussion:

1. `Session::undo`/`redo` return the `RegistryDelta` ops they applied (today they are applied and
   discarded).
2. A backward projection turns those ops into `EditorDelta`s: slot and export ops via single-input
   `to_runtime`; `AddNode`/`AddNetwork` ops materialize nodes from the op payloads (reverses carry
   full snapshots, so nothing is missing); attribute ops collapse into one `NodeMetadata` delta
   per touched node, read from the post-move registry through the existing `to_runtime` metadata
   path; global-to-local IDs resolve via the stashed `original_node_id` attributes.
3. `NodeNetworkInterface::apply_deltas(&[EditorDelta])` maps each variant onto existing mutation
   primitives plus a shared cache-invalidation epilogue. Applying must not re-record into the
   pending buffer (the ops are already history), so apply runs with recording suppressed; that
   suppression flag is the one new piece of interface state.

Rollout decision: **debug-compare**. Legacy snapshot undo remains authoritative while the
storage-driven path runs in parallel, applying to the same interface state and comparing against
the snapshot result; divergence logs in release and panics in tests, mirroring the existing
round-trip soak conventions. Snapshots are deleted only once the compare has been quiet across the
test suite and real use.

Known open points for that PR:

- Compare granularity: full interface equality per undo step is the strongest check and is likely
  affordable at undo frequency; decide whether metadata-only divergence should fail equally hard.
- Selection and view state are not in the registry, so storage-driven undo must leave them to the
  existing selection-history machinery rather than expecting them back from deltas.
- The same backward projection and apply pair is the future collab receive path; keep signatures
  free of undo-specific assumptions.
