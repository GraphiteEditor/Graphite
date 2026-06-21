# Summary

A document format (`.gdd`) for Graphite that decouples on-disk layout from the editor's in-memory runtime types. The format is a flat node registry plus a tree of operation-based CRDT deltas. The same delta type drives history, undo/redo, concurrent multi-user editing, migrations, and incremental compilation.

# Motivation

A delta-based, runtime-independent storage format addresses four problems with the legacy `.graphite` format (bincode/JSON of the editor's runtime structs):

- **Scattered migrations.** Three coexisting legacy mechanisms — global string replacement on serialized JSON (`document_migration_string_preprocessing`), per-field `#[serde(alias = ...)]` / `deserialize_with` on runtime structs, and post-deserialize fixups (`migrate_path_modify_node`, `migrate_node`) — each requires keeping old runtime shapes alive in the codebase.
- **Snapshot undo/redo.** `document_undo_history: VecDeque<NodeNetworkInterface>` clones the whole interface on every gesture.
- **No concurrent editing path.** Online multi-user editing and offline merge are blocked by the snapshot model.
- **Recompiled-from-scratch graphs.** No diff signal to drive incremental compilation.

A single delta representation unifies the data needed to fix all four: history step, CRDT op, migration unit, and compilation invalidation signal.

# Guide-level explanation

A document is a `Registry` plus a tree of operations applied to it.

## Registry

The `Registry` is a **flat** node graph. All nodes from all nested networks live in a single map; each node carries a back-pointer to its network. Networks themselves only store their list of exports. Proto-node declarations are not a separate table — they are content-addressed resources like any other (see [Resources](#resources)), referenced by `ResourceId`.

```rs
pub struct Registry {
    pub node_instances: HashMap<NodeId, Node>,                 // all nodes, flat
    pub networks: HashMap<NetworkId, Network>,                  // exports + per-network attrs
    pub exported_nodes: Vec<NodeId>,                            // library API surface
    pub peer_users: HashMap<PeerId, UserId>,                    // per-device → per-human identity
    pub resources: ResourceStore,                               // content-addressable resources (images, fonts, declarations)
    pub attributes: Attributes,                                 // document-level metadata
}

pub struct Node {
    pub implementation: Implementation,     // ProtoNode(ResourceId) or Network(net)
    pub inputs: Vec<InputSlot>,
    pub inputs_attributes: Vec<Attributes>,
    pub attributes: Attributes,
    pub network: NetworkId,
}

pub struct InputSlot {
    pub input: NodeInput,
    pub timestamp: TimeStamp,
}

pub struct Network {
    pub exports: Vec<ExportSlot>,
    pub attributes: Attributes,              // per-network ui::* (navigation, previewing)
}

pub struct ExportSlot {
    pub target: Option<NodeInput>,           // None = removed/empty
    pub timestamp: TimeStamp,
}

pub const ROOT_NETWORK: NetworkId = 0;
```

`peer_users` records the append-only `PeerId → UserId` mapping written by each device's first contribution (see [Concurrency model](#concurrency-model--cmrdt)).

The renderable graph lives in `networks[&ROOT_NETWORK]`. By convention the renderer consumes slot 0 of its exports; the editor can pick a different slot via type-based heuristics or user choice.

## Two exports concepts

- **`Network.exports`** — the outputs of a callable network. Used by parent networks and (on `ROOT_NETWORK`) by the renderer. High-frequency edits.
- **`Registry.exported_nodes`** — the document's library API: nodes an importing document can reference. A node exposed here may itself be backed by a network via `Implementation::Network`. Library metadata (display name, category, ...) lives as `library::*` attributes on the referenced node. Low-frequency edits.

Library import (how `.gdd` files reference each other and surface library nodes) is the subject of a follow-up RFC.

## Attributes — the type-erased metadata bucket

All metadata that isn't structural — node positions, display names, `call_argument` overrides, visibility, `context_features`, locked/pinned flags, input type hints, reflection metadata — lives in a single `Attributes` bucket per node, per input, and at the document level:

```rs
pub struct Value {
    pub value: serde_json::Value,
    pub timestamp: TimeStamp,
}

pub type Attributes = HashMap<String, Value>;
```

Keys are namespaced (`ui::position`, `compute::call_argument`, `library::display_name`, ...). Values are JSON; the per-value `TimeStamp` drives LWW on concurrent edits.

Type-erasure exists for migrations: storage data can be transformed without keeping old Rust struct shapes alive just to deserialize them.

## Deltas

A `RegistryDelta` is one atomic change to the registry, simultaneously a history step, a CRDT op to broadcast to peers, and a recompilation signal:

```rs
pub enum RegistryDelta {
    AddNode      { node_id: NodeId, node: Node },
    RemoveNode   { node_id: NodeId, snapshot: Node },
    ChangeNodeInput          { node_id: NodeId, input_idx: usize, new_input: NodeInput },
    ChangeNodeAttribute      { node_id: NodeId, delta: AttributeDelta },
    ChangeNodeInputAttribute { node_id: NodeId, input_idx: usize, delta: AttributeDelta },
    SetExport     { network: NetworkId, slot: u32, target: Option<NodeInput> },
    ChangeNetworkAttribute  { network: NetworkId, delta: AttributeDelta },   // per-network ui::nav::*, ...
    AddNetwork    { network: NetworkId, contents: Network },
    RemoveNetwork { network: NetworkId, snapshot: Network },
    SetExportedNodes        { nodes: Vec<NodeId> },
    ChangeDocumentAttribute { delta: AttributeDelta },
    RegisterPeer            { peer: PeerId, user: UserId },
    // Resources (incl. proto-node declarations):
    SetResourceHash { id: ResourceId, hash: Option<ResourceHash> },     // LWW on the resolved hash
    AddSource       { id: ResourceId, key: SourceKey, source: Value },  // add-wins entry in the source chain
    RemoveSource    { id: ResourceId, key: SourceKey },
    AddResource     { id: ResourceId, entry: ResourceEntry },           // whole-entry; reverse of RemoveResource
    RemoveResource  { id: ResourceId, snapshot: ResourceEntry },        // snapshot for O(1) reverse
}

/// `value: None` is the removal case. Timestamp lives on the wrapping `Delta`.
pub struct AttributeDelta {
    pub key: String,
    pub value: Option<serde_json::Value>,
}
```

Each delta is wrapped with metadata for history, identity, and causality. `Rev` is content-addressed: `blake3` truncated to 128 bits of `(parents, author, timestamp, delta_type)`, so identical content always produces the same `Rev` and concurrent retirements that converge collapse by construction.

```rs
pub type Rev = u128;

pub struct Delta {
    pub id: Rev,
    pub parents: Vec<Rev>,           // multi-parent for JJ-style merges
    pub author: PeerId,
    pub timestamp: TimeStamp,
    pub delta_type: RegistryDelta,
    pub reverse: RegistryDelta,      // precomputed for undo; excluded from id
    pub attributes: Attributes,      // mutable local annotations; excluded from id
}
```

One timestamp per `Delta` applies to every LWW-eligible write inside its `delta_type` — slot writes, attribute writes, and whole-list writes all read the same `Delta.timestamp`.

`Delta.attributes` is a type-erased annotation bucket (same shape as the registry's attribute buckets) for mutable, local-only labels — the `compute::gesture_end` marker that bounds undo units, and later commit messages. It is **excluded from `id`** so annotating a delta never changes its content-addressed identity; an inline write sets it before the delta's history frame is persisted, while a later relabel rewrites that frame.

## History as a tree

History is a multi-parent DAG. Branching is implicit: every concurrent or out-of-sync edit creates a branch by virtue of sharing a parent with another delta. A user's first commit after observing remote work adds the remote tip as an additional parent, so merges ride on the user's own edit rather than introducing phantom merge commits.

```
              D1 ── D2 ── D3        (one user's session)
             /
   ── root ──
             \
              D4 ── D5              (another peer, branched at root)
```

Linear undo is the common case; branching falls out naturally when two peers (or two windows on one machine) edit from the same parent. A history UI lets users navigate this tree to recover from convoluted undo/redo sessions or revisit past exploration. History compression collapses similar consecutive deltas (e.g., three sequential "move shape" ops) into a single coarser delta.

## Two-tier history: hot ops and retired commits

History has two tiers:

- **Hot ops** — speculative, broadcast per-keystroke for live collaboration. Carry only a Lamport timestamp; no parents, no content-addressed `Rev`. Live in `Document.hot_log`, GC'd at retirement, persisted as a sidecar for crash recovery. May pass through non-compiling intermediate states.
- **Retired commits** — coarser `Delta`s produced by retirement. Every retired commit compiles in the leader's local view. Content-addressed, multi-parent, durable, browseable, replayable.

A leader-elected peer periodically retires a window of hot ops into one or more semantically-equivalent retired commits (one per logical `(node, field)` group, not one giant commit per window). Retired commits use a single retirement timestamp for every field they write; the original hot-op timestamps are discarded. Leader election is gossip-based — lowest `PeerId` among peers whose `retirement_tip` matches the session max — and best-effort: there is no quorum, since content-addressed `Rev`s make concurrent retirements that converge dedupe by construction.

Do/undo pair collapse only happens when both land in the same retirement window, subject to dependency closure (collapse must not orphan a reference to `X`). The undo/redo mechanism itself is described below.

Solo retirement is the same mechanism with a session of one — history compaction during solo editing falls out for free.

## Undo/redo

Undo/redo operate on the delta history rather than full-interface snapshots. A commit's undo behavior depends on whether it has been broadcast to other peers, tracked by `last_broadcast_rev: Option<Rev>` on `Document` (the latest commit shared with at least one peer; `None`, and thus the entire history, during solo editing):

- **Silent zone** — commits after `last_broadcast_rev`. No other peer has seen them, so they can be rewound in place.
- **Published zone** — commits at or before `last_broadcast_rev`. Shared history is never rewound; undoing one is a *new* forward commit applying the inverse with a fresh timestamp, so concurrent peers converge by LWW.

The silent zone is the implemented path (solo editing has no transport yet); the published-zone forward-undo lands with collaboration.

**Silent-zone cursor.** `head: Rev` is a movable pointer into the append-only DAG. Undo/redo move it; they never delete deltas (that would make redo impossible and discard branch history). The extra state is a redo stack `Vec<Rev>` — the checkpoints the user has undone past — because the DAG alone can't say which child a `head` was undone *from*. New state persists in `session.json` alongside `head`, so redo survives reopen. A new edit while the redo stack is non-empty clears it (the undone-forward branch stays physically in the DAG but is no longer reachable via redo).

**Gestures, not deltas.** One user action retires into several deltas (one per `(node, field)` group), so undo steps per *gesture*: the last delta of each gesture is tagged with the `compute::gesture_end` attribute, and undo reverts deltas walking the first-parent chain until the parent is a `gesture_end` boundary or the root. The starting `head` (the checkpoint) is pushed to the redo stack; redo re-applies forward to it.

**Force-apply.** Rewinding re-applies each delta's precomputed `reverse` (for redo, the forward `delta_type`). These carry the *original* timestamp, which would tie — and so lose — the LWW arms' strict `>` comparison, since the forward op already stamped each field at that timestamp. In the single-writer silent zone the rewind value is authoritative, so silent undo/redo apply in a **force** mode where LWW arms assign unconditionally and structural ops are idempotent. Undo and redo are symmetric (force-reverse, force-forward), so no clock advances and identities are unchanged.

**Two registries.** Computing a correct `reverse` for an LWW field means reading the field's *pre-op* value. But staged edits apply to the live registry immediately (for responsiveness), so by retirement time it already holds the *post*-op value. `Document` therefore keeps two registries: a **working** registry (committed state plus live un-retired ops, what reads and the cursor see) and a **retired snapshot** (committed deltas only). Retirement computes reverses against and forward-applies to the snapshot, so the reverse captures the true prior value; the working registry already reflects the ops and is left as-is. When there are no un-retired ops the two are equal *by value* (their LWW field timestamps can differ, since retirement re-stamps the snapshot at a fresh time); undo/redo restore that equality by resyncing the snapshot to the rewound working registry.

## Concurrency model — CmRDT

The format uses an operation-based CRDT. The transport layer delivers ops in causal order exactly once (TCP plus the multi-parent chain in each `Delta`); the storage layer assumes this and requires only that concurrent op pairs commute. It does not need idempotency, state-merge, or out-of-order replay.

Graph-shape invariants (the graph remaining a DAG, the result compiling) are best-effort: conflicts that produce a non-compiling graph surface as wiring or type errors rather than being masked by the CRDT.

Identity is two-tier: `PeerId` is per-device (stable per `(device, document)`, used for CRDT tiebreaking and `NodeId` scoping); `UserId` is per-human (stable across devices, used for identity display and undo-chain walking). Each device's first contribution emits `RegisterPeer { peer, user }`, which writes an append-only entry to `Registry.peer_users`. Causal delivery guarantees the registration arrives before any of that peer's other ops.

## Editor pipeline

The editor operates on its existing runtime types. Storage is a serialization layer for persistence, sync, and history:

```
                ┌─────────────────────────────────────────┐
                │ Editor (runtime)                        │
                │  NodeNetworkInterface                   │
                │   ├── NodeNetwork  (compute graph)      │
                │   └── NodeNetworkMetadata  (editor UI)  │
                └─────────────────────────────────────────┘
                          ▲                  │
                          │ to_runtime       │ from_runtime
                          │                  ▼
                ┌─────────────────────────────────────────┐
                │ Storage layer  (graph-storage crate)    │
                │  Registry, RegistryDelta, Document      │
                └─────────────────────────────────────────┘
                                   │
                                   ▼
                ┌─────────────────────────────────────────┐
                │ On-disk  (.gdd container)               │
                │  named payloads: manifest, document,    │
                │  history, resources/<hash>              │
                │  served by a Container backend          │
                │  (folder, in-memory, OPFS), optionally  │
                │  encoded through an Archive codec       │
                │  (zip, xz)                              │
                └─────────────────────────────────────────┘
```

The runtime is the source of truth during editing. Conversion runs on save, on load, and across the sync boundary when broadcasting or receiving ops. The editor-facing handle is `Session` (`graph_storage::Session`); `Document` is internal. `Session::stage_from_runtime(&NodeNetwork, &dyn NodeMetadataSource)` is the entry point: it diffs the stored registry against a fresh conversion, ticks the clock once per emitted op, and applies each as a hot op on the hot log. The `Gdd` handle then persists the hot frames and retires them into durable history.

Staging and retirement are split so one undo gesture maps to one retired gesture. The editor's undo unit is one legacy transaction boundary, but a single user action (e.g. a tool drag) re-commits the runtime many times within one such boundary. So the editor *stages* on every commit (keeping the working registry and autosave current) and *retires the pending hot ops as one gesture* only at the undo-step boundary and before any undo/redo. (`commit_from_runtime` — stage and retire atomically — remains for one-shot callers.) Solo editing thus flows through the same hot-op-then-retire path collaboration uses, exercising it before any transport lands.

## On-disk container

A `.gdd` document is a collection of named byte payloads. A `Container` backend (loose folder, in-memory, OPFS in the browser) provides the path-keyed read/write surface; an `Archive` codec (zip, xz-compressed tarball) optionally encodes a container into a single byte stream for compact distribution. The same logical document can be saved as a loose folder for VCS-friendly checkouts or as an archive for shipping, without any change above the container layer.

The two concerns live in downstream crates: `document-container` defines the `Container` and `AsyncContainer` traits, the backends, byte ownership (mmap regions, owned buffers, external file mmaps via `mmap-io`), and the `Archive` trait. `document-format` defines the typed `Gdd` handle, the layout (logical-payload-name → in-container path), the data codec (JSON or binary), the manifest, and the save/load orchestration. `graph-storage` itself stays disk-unaware.

```
            ┌─────────────────────────────────┐
            │ editor                          │
            └─────────────────────────────────┘
                  │              │
                  ▼              ▼
            ┌───────────────┐  ┌──────────────────────────────┐
            │ graph-storage │  │ document-format              │
            │ (disk-unaware)│◀─│  Gdd handle, Layout, codec,  │
            └───────────────┘  │  ExportOptions               │
                               └──────────────────────────────┘
                                              │
                                              ▼
                               ┌──────────────────────────────┐
                               │ document-container           │
                               │  Container backends + Archive│
                               │  codecs (folder, memory,     │
                               │  OPFS / zip, xz)             │
                               └──────────────────────────────┘
```

Arrows are "depends on": the editor uses `Session` from `graph-storage` at runtime and `Gdd` from `document-format` on save/load; `document-format` serializes `graph-storage`'s types and delegates byte I/O to `document-container`; `graph-storage` and `document-container` are independent leaves.

A document contains:

- `manifest.json` — always JSON, the bootstrap file. Carries the magic identifier `"gdd"`, a single `u32` `format_version`, a stable `document_uuid`, the saving session's `PeerId`, editor and stdlib versions, an optional save timestamp, and a record of which payloads this save included (registry / history / embedded resources).
- `document.{json,bin}` — the serialized `Registry`. The codec is fixed per payload and recorded in the manifest (JSON for inspectable, MessagePack for compact; binary must be self-describing — see the codec rationale). Export reuses the working copy's recorded codecs rather than re-encoding.
- `history.{jsonl,frames}` — the serialized delta DAG, appended a record at a time. JSON history is line-oriented (one delta per line); binary history is length-prefixed MessagePack frames, the prefix guarding against a torn final frame from a crash.
- `resources/<hash>` — embedded resource bytes, keyed by `ResourceHash`.

The folder backend stores these as plain files on disk; an archive codec packs the same named entries into a single file.

```
            my-doc.gdd/
            ├── manifest.json
            ├── document.json
            ├── history.jsonl
            └── resources/
                ├── 7f3a...
                └── 2c91...
```

The `Gdd` handle owns the loaded bytes and exposes them as zero-copy slices. On the folder backend, reads are direct mmap references; loading from an archive decompresses once on open into an in-memory backend. The working copy is mutated continuously (autosave); `export(dest, format, options, byte_store)` produces a separate artifact through an `ExportFormat` (`Folder`/`Zip`/`Xz`) without mutating the handle.

`ExportOptions` controls scope: `include_registry` (skip = rebuild from history on load), `include_history` (skip = state-only snapshot), and `embed_all_resources`. These compose freely except that `include_registry: false && include_history: false` is rejected. The `byte_store` resolves resource bytes the working copy doesn't physically hold (in the editor they live in the app-global cache). `Embedded`-sourced resources are always materialized into the export's `resources/`; `embed_all_resources` additionally promotes link-only resources (`Url`/`FilePath`/`Font`) by prepending an `Embedded` source. That promotion is committed as real `AddSource` deltas on a throwaway session clone so the exported registry and history stay consistent; history is serialized in deterministic topological order, so identical delta sets export byte-identically.

## Resources

Everything content-addressable — raster images, fonts, embedded WASM, **and proto-node declarations** — is a resource. The storage `Registry` holds `resources: ResourceStore` (references only); the bytes live in a content-addressed byte store keyed by `ResourceHash`, owned by the caller (the app-global cache in the editor, the `Gdd` container for standalone/export), not by `graph-storage`.

```rs
pub type ResourceStore = HashMap<ResourceId, ResourceEntry>;

pub struct ResourceEntry {
    pub sources: Vec<(SourceKey, SourceValue)>,     // fallback chain, sorted by key, add-wins OR-set
    pub hash: Option<ResourceHash>,                  // resolved content hash (LWW)
    pub hash_timestamp: TimeStamp,
}

pub struct SourceKey   { pub priority: Priority, pub peer: PeerId }  // fractional priority + peer tiebreak
pub struct SourceValue { pub source: serde_json::Value, pub timestamp: TimeStamp }
```

A node references a resource by `ResourceId`; the entry maps it to a chain of `DataSource`s tried in order (`Embedded` bytes by hash, `FilePath`, `Url`, `Font`) plus the resolved `ResourceHash`. The chain is an **add-wins ordered OR-set**: each entry's `SourceKey` carries a fractional `Priority` so a peer can insert between two sources without renumbering, and concurrent insertions at the same priority converge via the `PeerId` tiebreak. The `hash` is **LWW** (content-derived, so concurrent resolves agree by construction).

Each `DataSource` is stored as `serde_json::Value` rather than a typed enum, with the same motivation as the `Attributes` bucket: type-erasure lets migrations restructure variants without keeping old enum shapes alive. `DataSource` stays typed at the runtime layer; conversion happens at the serialization boundary. Unknown variants are a hard error on load.

**Declarations as resources.** `Implementation::ProtoNode(ResourceId)` references a declaration resource. `from_runtime` serializes each `ProtoNode` through a self-describing `serde_json::Value` (MessagePack-encoded, via `encode_declaration`), hashes the bytes, derives the `ResourceId` from that hash (deterministic bootstrap; a future stable well-known-ID table would let the ID denote the function), and registers a `DataSource::Embedded` entry; the bytes go to the caller's byte store. `to_runtime` resolves declarations back via a `Declarations` (`ResourceId → ProtoNode`) map the caller builds from its byte store. The self-describing form keeps `ProtoNode`'s serde aliases working so the on-disk shape stays migratable.

A `NodeInput::Value` stores its `TaggedValue` as a self-describing `serde_json::Value` (the same type-erasure as `Attributes`/`DataSource`), so the `TaggedValue` serde aliases keep working and the on-disk shape stays migratable. Legacy documents with inline image `TaggedValue`s have those values extracted into resources at load time; new saves never embed inline image blobs in `NodeInput::Value`.

## Migrations

Migrations run on the type-erased `Registry`, after deserialization and before `to_runtime`. The pipeline reads the format version from the manifest, deserializes the registry with attributes as raw `serde_json::Value`, applies registered migrations scoped to the version range, and hands the result to `to_runtime`.

Migrations live in a dedicated crate so they are usable both from the editor and from a CLI for batch upgrades. A single global format version is used initially; per-library versioning is a future extension.

# Reference-level explanation

## Conversion: runtime ↔ storage

`from_runtime` flattens the recursive `NodeNetwork` into the flat `Registry`:

- Each node's path through the runtime nesting is hashed (blake3 truncated to 64 bits, with the document's `PeerId` mixed in) to produce a stable global `NodeId`. The original local ID is stashed in an attribute (`compute::original_node_id`) so the round-trip can rebuild the runtime's per-network local IDs. Subsequent live edits mint fresh peer-scoped IDs via `Document::next_node_id` (`blake3(peer, counter)`) instead of going through the path-hash bootstrap.
- Each nested `NodeNetwork`'s `NetworkId` is derived from the owning node's path (blake3 of `(peer, path)` with a `"network"` domain tag), not assigned by a traversal counter. This makes it stable across a `to_runtime` → `from_runtime` round trip — load-bearing because node paths (and thus node-ID hashes) include `NetworkId`s, so an unstable network ID would cascade into unstable node IDs and break re-commit after open. Aliasing (multiple nodes referencing the same network) is structurally supported by the storage model — `Implementation::Network(NetworkId)` is a reference — but the converter does not exploit it yet. Aliasing is fixed at the runtime layer first; the converter then preserves sharing without an explicit dedup pass.
- Non-structural `DocumentNode` fields (`call_argument`, `context_features`, `visible`, `skip_deduplication`, ...) become entries in the node's `attributes`. UI metadata from `DocumentNodeMetadata` (positions, display names, locked, pinned, ...) flows through the same bucket under `ui::*` keys.

`to_runtime` is the inverse: rebuild local IDs from the stashed attribute, restore typed fields from attribute values, follow `Implementation::Network` references to recursively materialize nested networks, and resolve `Implementation::ProtoNode(ResourceId)` against a `Declarations` map (`ResourceId → ProtoNode`) the caller supplies from its byte store. Since `graph-storage` is byte-unaware, `to_runtime` takes the resolved declarations as a parameter rather than reaching for bytes itself.

## Slots — inputs and exports

`Vec<InputSlot>` and `Vec<ExportSlot>` are positionally indexed at the storage layer. Each slot carries its own `TimeStamp`, giving LWW per slot on concurrent edits.

`ExportSlot` is sparse: `target == None` means the slot has been removed. `InputSlot` is dense. The runtime conversion compacts exports into a dense `Vec<NodeInput>` (preserving the runtime's "remove an export shifts later positions" semantics) and strips input timestamps.

Because inputs are stamped, `NodeInput::Node` references are set directly via `ChangeNodeInput` — there is no add/remove rewire workaround.

## CmRDT semantics

- **Timestamps.** `TimeStamp = (u64, PeerId)` — a Lamport counter with a peer-ID tiebreak. Comparison is lexicographic. Wall-clock time is not used.
- **NodeId identity.** Every new `AddNode` issues a peer-scoped ID, so concurrent creates cannot collide.
- **Causal delivery.** `apply_delta` requires every entry in `delta.parents` is already in local history. The storage layer does not buffer; out-of-order delivery is a transport concern. New peers initialize via snapshot transfer (`Registry` + history) before streaming deltas.
- **Removal.** Physical, no tombstones. If a later op targets an absent node or network, the receiver replays the most recent `AddNode` / network creation from history before applying. `RemoveNode` and `RemoveNetwork` each carry a `snapshot` of the removed entity so their reverse can rebuild in O(1) without re-walking history — required because retirement recomputes an op's reverse *after* the hot op already applied the removal, when the live entity is gone. Removal is therefore non-durable under concurrent edits: any concurrent reference to a removed node revives it.
- **LWW primitives.** Per-input (`InputSlot.timestamp`), per-export-slot (`ExportSlot.timestamp`), per-attribute-value (the `TimeStamp` in `Attributes`), and whole-list for `SetExportedNodes` via a sidecar timestamp in `Registry.attributes` under `library::exported_nodes_ts`. The timestamp driving every LWW arm comes from the wrapping `Delta`; `AttributeDelta` carries `value: Option<_>` so a single shape covers both `Set` (`Some`) and `Remove` (`None`) and `Set` vs. `Remove` has a defined winner.
- **Resources.** A resource's `hash` is LWW (content-derived, so concurrent resolves agree). Its source chain is an add-wins ordered OR-set keyed by `SourceKey` (fractional priority + peer tiebreak): concurrent `AddSource`s at distinct keys all survive; a re-add at the same key is LWW. Whole-resource `AddResource`/`RemoveResource` mirror the node/network add-remove pairs (`RemoveResource` snapshots the entry for O(1) reverse).

The CRDT does not mask graph-shape conflicts. Concurrent same-slot `SetExport`s with different targets resolve by LWW, but the resulting wiring may be wrong; downstream consumers see it as a compile or wiring error.

## History storage

`HashMap<Rev, Delta>` plus a `head: Rev` (the local cursor; advances only on local commits) and a `hot_log: Vec<HotOp>` (in-flight unretired ops). Walking history follows `delta.parents`; the default walk follows the first parent to reconstruct a single peer's local chain. Branches are siblings under a shared parent; merges aren't modeled as nodes — they're implicit in a delta listing multiple parents.

## Editor metadata

`DocumentNodePersistentMetadata` and `NodeNetworkPersistentMetadata` from the runtime — display names, locked/pinned, navigation/PTZ state, selection undo/redo stacks, layer/node type metadata — flow through the storage `Attributes` bucket under `ui::*` keys. Transient runtime caches (`DocumentNodeTransientMetadata`, click targets, resolved types, `OriginalLocation`) stay runtime-only and are not stored.

Document-scoped editor settings (viewport view, render mode, overlay/ruler visibility, snapping, collapsed layers) ride the document-level `Registry.attributes` under `ui::doc::*` keys, supplied through `NodeMetadataSource::document_attributes`. This is what makes the `.gdd` a lossless replacement for the legacy format's document-handler fields.

# Drawbacks

- **Diffing two full `Registry`s on every autosave is O(N) in document size.** The interim cost of treating storage as a serialization layer derived from the runtime; currently triggered at autosave boundaries (`commit_storage_snapshot`) rather than per gesture, and addressed long-term by computing deltas directly on runtime mutations.
- **Attributes as `serde_json::Value` carry per-value overhead.** Mitigable with a typed fast path for hot keys without changing the design. They also force a self-describing codec, ruling out the most compact binary formats.
- **Single global format version is a sharp edge** when libraries diverge: a breaking change in one library bumps the version for documents that don't use it.
- **`RemoveNode` is non-durable under concurrency.** Any concurrent reference to a removed node revives it from history.

# Rationale and alternatives

**Delta-based vs. cleaner snapshot format.** A delta is the right unit for history, CRDT sync, and incremental compilation. Picking one representation for all three eliminates conversion seams between subsystems that need to interoperate.

**CmRDT vs. state-based CRDT or OT.** State-based CRDTs require a merge function and large state vectors. OT requires a central server to mediate transforms. CmRDT only requires per-op commutativity plus a causal-order delivery layer, which the transport provides.

**Ad-hoc resurrection vs. tombstones.** Tombstones add a permanent footprint to the data model and a GC policy question. Resurrection reuses the history log already needed for undo as the recovery mechanism, keeping the live `Registry` lean. The cost is that `RemoveNode` is not durable under concurrent edits.

**Type-erased attributes vs. typed metadata fields.** Migrations operate on attribute values without keeping old Rust struct shapes alive. The cost is per-value overhead, mitigable without changing the model.

**Flat node storage vs. nested networks.** All CRDT ops target nodes via a single uniform `NodeId` address space regardless of nesting depth. A nested representation would require ops to carry a path, complicating commutativity.

**`.gdd` vs. reusing `.graphite`.** A distinct extension makes migration unambiguous and prevents older Graphite versions from trying to open a new-format file.

**One self-describing binary codec (MessagePack).** Every persisted body — deltas, the registry, node-input values, and `ProtoNode` declarations — is a type-erased `serde_json::Value`, so it needs a self-describing codec to deserialize and to keep the serde-alias migration path alive; MessagePack provides that at a few percent size cost. Hash preimages (`NodeId`, `Rev`, node-path hashes) use the same codec: a fixed serializer emits one deterministic byte form per value, which is all `blake3` needs, so the codec doubles as the canonical hash encoding without a second format.

**Source chain as a sorted `Vec` vs. `BTreeMap`.** `SourceKey` is a struct, so a `BTreeMap`-keyed chain can't serialize to JSON (string keys only). A sorted `Vec` of pairs keeps the same ordering and add-wins semantics losslessly across every codec.

# Future possibilities

- **Per-library format versioning** so a breaking change in one library doesn't bump the version for documents that don't use it.
- **History linearization** — prune unused branches from a convoluted tree to produce a clean undo/redo history.
- **Runtime-native deltas.** Move delta computation out of the storage layer into the runtime, eliminating per-edit `Registry` re-conversion.
- **Incremental compilation driven by deltas.** The compiler consumes runtime deltas and recompiles only changed regions.
- **Runtime-level aliasing for shared node-network definitions.** Storage already supports `Implementation::Network` as a reference; once the runtime supports sharing natively, the converter preserves it.
- **Online migration service** — active editors drop migrations older than some threshold; old documents go through a remote upgrade pipeline first.
- **Distributed / signed history.** Content-addressed `Rev` plus signing enables multi-author provenance and verifiable history.
- **Libraries as files** — a follow-up RFC will specify how `.gdd` files act as importable libraries via `Registry.exported_nodes`.
