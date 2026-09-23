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
   site to what it touches. **Open fork, see "Emission" below; do not start without a decision.**
4. Staging: `construct_batch` into `Session::stage_computed_ops` via `GddV1::stage_runtime_deltas`,
   with the whole-document diff as the desync fallback and the parity soak in validate mode.
5. Compiler consumption: replace the per-update whole-network clone in `GraphUpdate` with the
   `RuntimeDelta` stream reconciling a mirror on the runtime thread, using apply-reported change
   as the invalidation signal.

### Emission: the open fork on phase 3

Mutators do not yield deltas today; the delta is recovered by diffing the whole registry at commit
time (`Session::stage_from_runtime`). Three shapes, none chosen:

- **(a) Emit at the `store.rs` primitives.** Every write already funnels through them, so emission
  cannot be forgotten. But they are finer-grained than `RuntimeDelta` (`insert_input_slot`, not
  `SetInput`), so peers would receive a different, lower-level language than `EditorDelta` defines.
- **(b) Emit at the mutator level.** Matches `RuntimeDelta` granularity exactly, but must be
  hand-added to roughly forty public mutators and can silently be forgotten in a new one.
- **(c) Keep diffing, but scope the diff to the batch's touched set** rather than the whole
  document. Reuses `compute_deltas`, cannot drift, and `verify_round_trip` already proves the diff
  faithful. Cost: no semantic delta, a move stays a remove-plus-add. Fine for peers, since
  `RegistryDelta` is the wire format anyway; not what incremental compilation wants.

Rough read at the time of writing: (c) if peer sync is the driver, (b) if incremental compilation
is. (a) is the only structurally un-forgettable one but changes what a delta means.

Sequencing once that is decided: the **batch boundary** comes first (a scope a run of writes
happens inside, collecting emitted deltas and the dirty set, flushed once at the end), because both
emission and the deferred invalidation flush need it. Decide explicit `finish()` versus `Drop`;
`Drop` cannot report failure and interacts awkwardly with `transaction_modified`. Then `apply` as
mechanism only, which is the smaller half because `store.rs` is already total and already the
single writer. Gate for `apply`: `validate_invariants()` empty afterwards, and applying a
mutation's own delta to a clone of the interface reproduces that interface.

## Interface restructure state (branch `runtime-deltas-2`, 2026-09-22)

The restructure that prepares `NodeNetworkInterface` for the above. Twenty-two commits, rebased
onto `origin/master` at `d7ae6029e0`, nothing pushed.

### Module shape

`editor/src/messages/portfolio/document/utility_types/network_interface/`

| file | role | lines, start to now |
|---|---|---|
| `store.rs` | the only writer of both parallel trees | new, 428 |
| `mutations.rs` | policy: validation, layout decisions | 1629 to 1271 |
| `layout.rs` | positioning policy | 1295 |
| `caches.rs` | transient cache slots | 1258 to 876 |
| `queries.rs` | reads | 1229 to 966 |
| `geometry.rs` | click targets, ports, widths | new, 352 |
| `frontend.rs` | frontend projections | new, 181 |
| `invalidation.rs` | named invalidation rules | new, 40 |

`network: MemoNetwork` and `network_metadata: NodeNetworkMetadata` are parallel trees keyed by the
same `NodeId`s. `store.rs` is their single writer, so every write keeps them in step by
construction; `validate_invariants` (test-only, `validation.rs`) checks it.

The resolved cursors `NodeMut` / `NetworkMut` hold disjoint field borrows and therefore cannot call
back into the interface. That is deliberate: it forces what a write invalidates to be *declared* by
the caller rather than performed inside the write.

### Identity

Storage `NodeId` resolution goes through exactly one place, `NodeIds::resolve` in
`document/graph-storage/src/from_runtime.rs`: it prefers the identity the metadata source holds and
falls back to `blake3(peer, NodePath)`, a hash of the node's location. Every naming site routes
through it, including the node's own registry key, input references, scope injections,
`PathResolver` and `ScopedConversion`.

`NodeMetadataEntry` carries `storage_id` and is emitted for every node (`is_empty()` is gone).
`build_interface_from_storage` pins it onto `DocumentNodePersistentMetadata::storage_id`, so a
document that round-trips through storage keeps its identities.
`rebuilt_interface_keeps_storage_identities` (in `storage_tests/metadata_tests.rs`) asserts a
re-conversion under a *different peer* reproduces the same ids.

**Minting is deliberately not done** (Dennis chose this on 2026-09-22), because:

- `NodeTemplate` drops `storage_id` in both `from_parts` and `into_parts`, so a remove/insert
  template round trip loses the pin. Making the template carry it collides with its other role as
  the clipboard and definition type.
- Grouping, ungrouping and paste all go through `copy_nodes`, which assigns *new runtime* `NodeId`s
  to everything, so a node entering a group is already a new node at the runtime level.
- Nothing else in a session changes a node's location while keeping its runtime id, so a mint is
  currently unobservable.

Agreed sequencing: make grouping preserve runtime ids **first**, then mint. The north star stays
"the gdd native path is minting, the hash is only polyfill".

### Open items

1. **Deferred dirty-set flush** (step 4 tail). Blocked on the batch boundary above; do not invent
   one just to have somewhere to flush into.
2. **Deleting the `queries.rs` wrappers** (step 5). Standing objection: roughly forty one-line
   delegations against migrating hundreds of call sites. Not done; reopen only if Dennis disagrees.
3. **Move the module up beside `node_graph/`** (step 6). Pure file shuffle, do it whenever.
4. **Grouping preserving runtime ids**, the prerequisite for minting.

### Complexity

Fixed: `collect_removal_closure` was O(nested networks x all nodes), rescanning `node_instances`
per network; it now uses a `NodesByNetwork` index built once. `construct_resource_removals` was
O(candidates x (nodes + exports)); it now builds a `still_referenced` set in one pass. Dead
plumbing removed along the way (`batch_removed_networks` was computed and never read; `construct`
took `batch_removed_nodes` only to discard it).

Still open, design-level rather than local bugs:

- **`position()` is not memoized.** Each call walks downstream to the first stored absolute
  position, so computing positions for many nodes is O(n x depth). A document that is one tall
  stack makes `nodes_sorted_top_to_bottom`, and therefore every `shift_nodes`, quadratic. Fixing it
  means caching resolved positions against an invalidation epoch.
- **`settle_drag_offsets` and `push_stack_down_to_fit` move one grid row per iteration**, calling
  `check_collision_with_stack_dependents` / `vertical_shift_with_push` each time, so cost scales
  with the distance moved times a stack walk rather than with node count. Inherent to the
  incremental-push model.

### Latent bugs found and left alone

- `reposition_disconnected_upstream` (`layout.rs`): the nested `if` is load-bearing. A layer left
  feeding the bottom of a *non-layer* currently does nothing, neither stacks nor goes absolute.
  Flattening it changes behaviour, so it was moved verbatim. Looks like a bug, not a deliberate
  case.
- `add_export` reaches into `encapsulating_network_metadata_mut(..).transient_metadata` directly to
  unload two caches, bypassing `invalidation.rs`. It cannot simply call
  `unload_all_nodes_bounding_box`, which *also* invalidates import/export ports, so substituting
  would add an invalidation. Needs a decision.
- `add_export` / `add_import` invalidate almost exactly what `finish_signature_edit` does, but in a
  different order, with different `load_structure` gates, and `add_export` must handle the root
  network. Merging them would be behaviour-changing, not a refactor.
- The dead `lowest_upstream_node_height` loop removed from `move_layer_to_stack` had a `log::error!`
  plus `return` that aborted the whole move as a side effect of computing a value nobody read.
  Removing it narrows that abort; reinstate as an explicit check if it turns out to matter.

### Gates

- `cargo check -p graphite-editor --all-targets`
- `cargo clippy --workspace --all-targets`, must be **zero** warnings; pre-existing ones were fixed
  too, on Dennis's instruction.
- `cargo fmt --all` after every change.
- `cargo test -p graphite-editor` (225 pass, 1 ignored);
  `cargo test -p document-graph-storage` (57); `-p document-format -p document-container` (48).
- Wasm gate, needed only when touching `document-container`, `document-format`, `graph-storage` or
  the editor `future` module: `nix develop --command cargo run build web debug`, about two minutes.
  `wasm-pack` is not on the bare PATH; the flake provides it.

The sharp round-trip gate for identity work is `recommit_after_open_is_stable` in
`storage_tests/round_trip_tests.rs`, plus `verify_round_trip` in `document_history.rs`, which
panics under `#[cfg(test)]` on registry value drift after commit.

### Method note

Extracting named pieces from this code costs roughly as many lines as it saves once signatures, doc
comments and the `-> bool` plumbing that preserves abort-on-error behaviour are counted. Across
three refactors the net was about -38, +5 and +35 lines. Promise readability, not line reduction. A
pedantic clippy sweep on the editor is not worth running: roughly five hundred warnings
workspace-wide, dominated by house-style choices (`&NodeId` parameters, `use super::*`). Take idiom
fixes only in code you are already touching.

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
