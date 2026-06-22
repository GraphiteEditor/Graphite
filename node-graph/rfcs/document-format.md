# Summary

A document format (`.gdd`) for Graphite that decouples on-disk layout from the editor's in-memory runtime types. The format is a flat node registry plus a tree of operation-based CRDT deltas. The same delta type drives history, undo/redo, concurrent multi-user editing, migrations, and incremental compilation.

# Motivation

A delta-based, runtime-independent storage format addresses four problems with the legacy `.graphite` format (bincode/JSON of the editor's runtime structs):

- **Scattered migrations.** Three legacy mechanisms coexist: global string replacement on serialized JSON (`document_migration_string_preprocessing`), per-field `#[serde(alias = ...)]` and `deserialize_with` on runtime structs, and post-deserialize fixups (`migrate_path_modify_node`, `migrate_node`). Each requires keeping old runtime shapes alive in the codebase.
- **Snapshot undo/redo.** `document_undo_history: VecDeque<NodeNetworkInterface>` clones the whole interface on every gesture.
- **No concurrent editing path.** Online multi-user editing and offline merge are blocked by the snapshot model.
- **Recompiled-from-scratch graphs.** No diff signal to drive incremental compilation.

A single delta representation unifies the data needed to fix all four: history step, CRDT op, migration unit, and compilation invalidation signal.

# Guide-level explanation

A document is a `Registry` plus a tree of operations applied to it.

## Registry

The `Registry` is a **flat** node graph. All nodes from all nested networks live in a single map, and each node carries a back-pointer to its network. Networks themselves only store their list of exports. Proto-node declarations are not a separate table. They are content-addressed resources like any other (see [Resources](#resources)), referenced by `ResourceId`.

```rs
pub struct Registry {
    pub node_instances: HashMap<NodeId, Node>,                 // all nodes, flat
    pub networks: HashMap<NetworkId, Network>,                  // exports + per-network attrs
    pub resources: ResourceStore,                               // content-addressable resources (images, fonts, declarations)
    pub peer_users: HashMap<PeerId, UserId>,                    // per-device → per-human identity
    pub attributes: Attributes,                                 // document-level metadata
}

pub struct Node {
    pub implementation: Implementation,     // ProtoNode(ResourceId) or Network(net)
    pub inputs: Vec<InputSlot>,
    pub attributes: Attributes,
    pub network: NetworkId,
}

pub struct InputSlot {
    pub input: NodeInput,
    pub timestamp: TimeStamp,
    pub attributes: Attributes,             // per-input metadata, LWW per key
}

pub struct Network {
    pub exports: Vec<ExportSlot>,
    pub attributes: Attributes,              // per-network ui::* (navigation, previewing)
}

pub struct ExportSlot {
    pub target: Option<NodeInput>,           // None = removed/empty
    pub timestamp: TimeStamp,
}

pub const ROOT_NETWORK: NetworkId = NetworkId(0);
```

`peer_users` records the append-only `PeerId → UserId` mapping written by each device's first contribution (see [Concurrency model](#concurrency-model-cmrdt)).

The renderable graph lives in `networks[&ROOT_NETWORK]`. By convention the renderer consumes slot 0 of its exports. The editor can pick a different slot via type-based heuristics or user choice.

## Two exports concepts

- **`Network.exports`** are the outputs of a callable network, used by parent networks and (on `ROOT_NETWORK`) by the renderer. High-frequency edits.
- **Exported nodes** are the document's library API: nodes an importing document can reference. A node exposed here may itself be backed by a network via `Implementation::Network`. Library metadata (display name, category, and so on) lives as `library::*` attributes on the referenced node. Low-frequency edits. The list is a document-level attribute (`Registry.attributes["exported_nodes"]`) rather than a dedicated field, so it rides the ordinary `ChangeDocumentAttribute` LWW path with its own per-key timestamp.

Library import (how `.gdd` files reference each other and surface library nodes) is the subject of a follow-up RFC. The `exported_nodes` attribute key is reserved but not yet read or written.

## Attributes: the type-erased metadata bucket

All metadata that is not structural lives in a single `Attributes` bucket per node, per input, and at the document level. That covers node positions, display names, `call_argument` overrides, visibility, `context_features`, locked/pinned flags, input type hints, and reflection metadata.

```rs
pub struct Value {
    pub value: serde_json::Value,
    pub timestamp: TimeStamp,
}

pub type Attributes = BTreeMap<String, Value>;
```

Keys carry a namespace where one applies, mostly the `ui::*` editor-metadata keys (`ui::position`, `ui::display_name`, and so on). Compute fields use bare keys (`call_argument`, `context_features`, `original_node_id`). Values are JSON, and the per-value `TimeStamp` drives LWW on concurrent edits.

Type-erasure exists for migrations: storage data can be transformed without keeping old Rust struct shapes alive just to deserialize them.

## Deltas

A `RegistryDelta` is one atomic change to the registry, simultaneously a history step, a CRDT op to broadcast to peers, and a recompilation signal:

```rs
pub enum RegistryDelta {
    AddNode      { id: NodeId, node: Node },
    RemoveNode   { id: NodeId, snapshot: Node },
    ChangeNodeInput          { id: NodeId, index: u32, new_input: NodeInput },
    ChangeNodeAttribute      { id: NodeId, delta: AttributeDelta },
    ChangeNodeInputAttribute { id: NodeId, index: u32, delta: AttributeDelta },
    SetNetworkExport         { id: NetworkId, index: u32, export: Option<NodeInput> },
    ChangeNetworkAttribute  { id: NetworkId, delta: AttributeDelta },   // per-network ui::nav::*, ...
    AddNetwork    { id: NetworkId, network: Network },
    RemoveNetwork { id: NetworkId, snapshot: Network },
    ChangeDocumentAttribute { delta: AttributeDelta },                  // incl. the exported_nodes list
    RegisterPeer            { peer: PeerId, user: UserId },             // self-inverse; see below
    // Resources (incl. proto-node declarations):
    SetResourceHash { id: ResourceId, hash: Option<ResourceHash> },     // LWW on the resolved hash
    AddSource       { id: ResourceId, key: SourceKey, source: serde_json::Value },  // insert/LWW entry in the source chain
    RemoveSource    { id: ResourceId, key: SourceKey },
    AddResource     { id: ResourceId, entry: ResourceEntry },           // whole-entry; reverse of RemoveResource
    RemoveResource  { id: ResourceId, snapshot: ResourceEntry },        // snapshot for O(1) reverse
    Merge           { extra_parents: Vec<Rev> },                        // joins divergent tips; registry no-op
    Other(serde_json::Value),                                           // forward-compatible escape hatch
}

/// `value: None` is the removal case. Timestamp lives on the wrapping `Delta`.
pub struct AttributeDelta {
    pub key: String,
    pub value: Option<serde_json::Value>,
}
```

Each delta is wrapped with metadata for history, identity, and causality. `Rev` is content-addressed: `blake3` truncated to 128 bits of `(parent, author, timestamp, kind)`, so identical content always produces the same `Rev` and concurrent retirements that converge collapse by construction.

```rs
pub struct Rev(pub NonZeroU128);

pub struct Delta {
    pub id: Rev,
    pub parent: Option<Rev>,         // primary parent; None for the root delta
    pub author: PeerId,
    pub timestamp: TimeStamp,
    pub kind: RegistryDelta,
    pub reverse: RegistryDelta,      // precomputed for undo; excluded from id
    pub attributes: Attributes,      // mutable local annotations; excluded from id
}
```

The history DAG is multi-parent, but a delta stores only its primary `parent`. Extra parents ride a dedicated `RegistryDelta::Merge { extra_parents }` op, which joins divergent tips into one node and is a registry no-op on replay. So a merge's identity is its parent set alone: two peers merging the same tips mint the identical `Rev` and it dedups.

Because `parent` is itself a `Rev`, an `id` transitively commits to the delta's whole ancestry, the same Merkle chaining a Git commit hash gives. A `Delta`'s `id` is therefore recomputable from its identity fields, and history loaded from an untrusted source is checked by rehashing every delta and rejecting any whose stored `id` does not match (the load path skips this for trusted local data, since the rehash is not free over a large history).

One timestamp per `Delta` applies to every LWW-eligible write inside its `kind`. Slot writes, attribute writes, and whole-list writes all read the same `Delta.timestamp`.

`Delta.attributes` is a type-erased annotation bucket (the same shape as the registry's attribute buckets) for mutable, local-only labels: the `interaction_end` marker that bounds undo units, and later commit messages. It is **excluded from `id`** so that annotating a delta never changes its content-addressed identity. An inline write sets it before the delta's history frame is persisted, while a later relabel rewrites that frame.

## History as a tree

History is a multi-parent DAG. Branching is implicit: every concurrent or out-of-sync edit creates a branch by virtue of sharing a parent with another delta. Divergent tips are rejoined by an explicit `Merge` delta listing the joined tips, which is a registry no-op (it only collapses the tips so `head` stays a single `Rev`).

```
              D1 ── D2 ── D3        (one user's session)
             /
   ── root ──
             \
              D4 ── D5              (another peer, branched at root)
```

Linear undo is the common case. Branching falls out naturally when two peers (or two windows on one machine) edit from the same parent. A history UI lets users navigate this tree to recover from convoluted undo/redo sessions or revisit past exploration. History compression (planned, not yet implemented) would collapse similar consecutive deltas (for example, three sequential "move shape" ops) into a single coarser delta.

## Two-tier history: hot ops and retired commits

History has two tiers:

- **Hot ops** are speculative, intended to be broadcast per-keystroke for live collaboration (broadcast transport is not yet implemented, so today they stay local). They carry only a Lamport timestamp, with no parents and no content-addressed `Rev`. They live in `Document.hot_log`, are GC'd at retirement, and are persisted as a sidecar for crash recovery. They may pass through non-compiling intermediate states.
- **Retired commits** are `Delta`s produced by retirement. Every retired commit compiles in the retiring peer's local view. They are content-addressed, durable, browseable, and replayable.

Retirement promotes a window of hot ops (those with timestamp at or before a cutoff) into retired deltas, re-applied with a single fresh retirement timestamp per field so LWW arms bump to `T_retire` and the original hot-op timestamps are discarded. Today retirement is one retired delta per hot op. The interfaces are in place for coarsening a window into fewer, semantically-equivalent commits (one per logical `(node, field)` group), but that grouping is not yet implemented.

In a collaborative session a leader-elected peer would own retirement. That election is designed to be gossip-based (the lowest `PeerId` among peers whose `retirement_tip` matches the session max) and best-effort, needing no quorum because content-addressed `Rev`s make concurrent retirements that converge dedupe by construction. The `retirement_tip` heartbeat field exists but is inert until broadcast transport lands. Today every session retires its own hot ops (see solo retirement below).

Once coarsening lands, do/undo pair collapse will only happen when both land in the same retirement window, subject to dependency closure (collapse must not orphan a reference to `X`). The undo/redo mechanism itself is described below.

Solo retirement is the same mechanism with a session of one, so history compaction during solo editing will fall out for free once coarsening is implemented.

## Undo/redo

Undo/redo operate on the delta history rather than full-interface snapshots. A commit's undo behavior depends on whether it has been broadcast to other peers, tracked by `last_broadcast_rev: Option<Rev>` on `Document` (the latest commit shared with at least one peer, or `None`, and thus the entire history, during solo editing):

- **Silent zone:** commits after `last_broadcast_rev`. No other peer has seen them, so they can be rewound in place.
- **Published zone:** commits at or before `last_broadcast_rev`. Shared history is never rewound. Undoing one is a *new* forward commit applying the inverse with a fresh timestamp, so concurrent peers converge by LWW.

The silent zone is the implemented path (solo editing has no transport yet). The published-zone forward-undo lands with collaboration.

**Silent-zone cursor.** `head: Option<Rev>` is a movable pointer into the append-only DAG (`None` on an empty document with no commits yet). Undo/redo move it but never delete deltas (that would make redo impossible and discard branch history). The extra state is a redo stack `Vec<Rev>`, the checkpoints the user has undone past, because the DAG alone cannot say which child a `head` was undone *from*. New state persists in `session.json` alongside `head`, so redo survives reopen. A new edit while the redo stack is non-empty clears it (the undone-forward branch stays physically in the DAG but is no longer reachable via redo).

**Interactions, not deltas.** One user action diffs into several deltas (one per changed field, slot, or attribute), so undo steps per *interaction* rather than per delta. The last delta of each interaction is tagged with the `interaction_end` attribute, and undo reverts deltas walking the first-parent chain until the parent is an `interaction_end` boundary or the root. The starting `head` (the checkpoint) is pushed to the redo stack, and redo re-applies forward to it.

**Force-apply.** Rewinding re-applies each delta's precomputed `reverse` (for redo, the forward `kind`). These carry the *original* timestamp, which would tie (and so lose) the LWW arms' strict `>` comparison, since the forward op already stamped each field at that timestamp. In the single-writer silent zone the rewind value is authoritative, so silent undo/redo apply in a **force** mode where LWW arms assign unconditionally and structural ops are idempotent. Undo and redo are symmetric (force-reverse, force-forward), so no clock advances and identities are unchanged.

**Two registries.** Computing a correct `reverse` for an LWW field means reading the field's *pre-op* value. But staged edits apply to the live registry immediately (for responsiveness), so by retirement time it already holds the *post*-op value. `Document` therefore keeps two registries: a **working** registry (committed state plus live un-retired ops, what reads and the cursor see) and a **retired snapshot** (committed deltas only). Retirement computes reverses against, and forward-applies to, the snapshot, so the reverse captures the true prior value, and the working registry already reflects the ops and is left as-is. When there are no un-retired ops the two are equal *by value* (their LWW field timestamps can differ, since retirement re-stamps the snapshot at a fresh time), and undo/redo restore that equality by resyncing the snapshot to the rewound working registry.

## Concurrency model: CmRDT

The format uses an operation-based CRDT. The transport layer delivers ops in causal order exactly once (TCP plus the parent links threading the `Delta` DAG). The storage layer assumes this and requires only that concurrent op pairs commute. It does not need idempotency, state-merge, or out-of-order replay.

Graph-shape invariants (the graph remaining a DAG, the result compiling) are best-effort. Conflicts that produce a non-compiling graph surface as wiring or type errors rather than being masked by the CRDT.

Identity is two-tier. `PeerId` is per-device (stable per `(device, document)`, used for CRDT tiebreaking and `NodeId` scoping). `UserId` is per-human (stable across devices, used for identity display and undo-chain walking). Each device's first contribution emits `RegisterPeer { peer, user }`, which writes an append-only entry to `Registry.peer_users`. Causal delivery guarantees the registration arrives before any of that peer's other ops. The mapping is permanent (first write wins, a conflicting re-registration errors, and an identical one is a no-op), so `RegisterPeer` is its own reverse: replaying it during undo is a no-op rather than needing a distinct removal variant.

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
                │  named payloads: manifest, registry,    │
                │  history, session, resources/<hash>     │
                │  served by a Container backend          │
                │  (folder, in-memory, OPFS), optionally  │
                │  encoded through an Archive codec       │
                │  (zip, xz)                              │
                └─────────────────────────────────────────┘
```

The runtime is the source of truth during editing. Conversion runs on save, on load, and across the sync boundary when broadcasting or receiving ops. The editor-facing handle is `Session` (`graph_storage::Session`), and `Document` is internal. `Session::stage_from_runtime(&NodeNetwork, &dyn NodeMetadataSource)` is the entry point: it diffs the stored registry against a fresh conversion, ticks the clock once per emitted op, and applies each as a hot op on the hot log. The `Gdd` handle then persists the hot frames and retires them into durable history.

Staging and retirement are split so that one undo gesture maps to one retired gesture. The editor's undo unit is one legacy transaction boundary, but a single user action (for example, a tool drag) re-commits the runtime many times within one such boundary. So the editor *stages* on every commit (keeping the working registry and autosave current) and *retires the pending hot ops as one gesture* only at the undo-step boundary and before any undo/redo. (`commit_from_runtime`, which stages and retires atomically, remains for one-shot callers.) Solo editing thus flows through the same hot-op-then-retire path that collaboration uses, exercising it before any transport lands.

## On-disk container

A `.gdd` document is a collection of named byte payloads. A `Container` backend (loose folder, in-memory, OPFS in the browser) provides the path-keyed read/write surface. An `Archive` codec (zip, xz-compressed tarball) optionally encodes a container into a single byte stream for compact distribution. The same logical document can be saved as a loose folder for VCS-friendly checkouts or as an archive for shipping, without any change above the container layer.

The two concerns live in downstream crates. `document-container` defines the `Container` and `AsyncContainer` traits, the backends, byte ownership (mmap regions, owned buffers, external file mmaps via `mmap-io`), and the `Archive` trait. `document-format` defines the typed `Gdd` handle, the layout (logical-payload-name to in-container path), the data codec (JSON or binary), the manifest, and the save/load orchestration. `graph-storage` itself stays disk-unaware.

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

Arrows mean "depends on". The editor uses `Session` from `graph-storage` at runtime and `Gdd` from `document-format` on save/load. `document-format` serializes `graph-storage`'s types and delegates byte I/O to `document-container`. `graph-storage` and `document-container` are independent leaves.

A document contains:

- `manifest.json` is always JSON, the bootstrap file. It carries the magic identifier `"gdd"` (the `format` field), a single `u32` `format_version`, a `document_id`, the editor and stdlib versions, and the per-payload codec table (`codecs`). It deliberately omits per-peer state: the saving peer's `PeerId` and the history cursor live in the session payload, not the manifest, so they travel with the local view rather than the shared document.
- `registry.{json,bin}` is the serialized `Registry`. The codec is fixed per payload and recorded in the manifest (JSON for inspectable, MessagePack for compact, and binary must be self-describing, as the codec rationale explains). Export reuses the working copy's recorded codecs rather than re-encoding.
- `history.{jsonl,frames}` is the serialized retired delta DAG, appended a record at a time. JSON history is line-oriented (one delta per line). Binary history is length-prefixed MessagePack frames, the prefix guarding against a torn final frame from a crash.
- `hot-log.{jsonl,frames}` is the un-retired hot ops, persisted as a sidecar for crash recovery and GC'd at retirement.
- `session.{json,bin}` is per-peer local state: this peer's `PeerId`, the history cursor (`head_rev`), the redo stack, `last_broadcast_rev`, and view settings. It is local-view state, not part of the shared document.
- `resources/<hash>` is embedded resource bytes, keyed by `ResourceHash`.

The folder backend stores these as plain files on disk, and an archive codec packs the same named entries into a single file.

```
            my-doc.gdd/
            ├── manifest.json
            ├── registry.json
            ├── history.jsonl
            ├── hot-log.jsonl
            ├── session.json
            └── resources/
                ├── 7f3a...
                └── 2c91...
```

The `Gdd` handle owns the loaded bytes and exposes them as zero-copy slices. On the folder backend, reads are direct mmap references, while loading from an archive decompresses once on open into an in-memory backend. The working copy is mutated continuously (autosave), and `export(dest, format, options, byte_store)` produces a separate artifact through an `ExportFormat` (`Folder`/`Zip`/`Xz`) without mutating the handle.

`ExportOptions` controls scope: `include_registry` (skipping it rebuilds from history on load), `include_history` (skipping it produces a state-only snapshot), and `embed_all_resources`. These compose freely except that `include_registry: false && include_history: false` is rejected. The `byte_store` resolves resource bytes the working copy does not physically hold (in the editor they live in the app-global cache). `Embedded`-sourced resources are always materialized into the export's `resources/`, and `embed_all_resources` additionally promotes link-only resources (`Url`/`FilePath`/`Font`) by prepending an `Embedded` source. That promotion is committed as real `AddSource` deltas on a throwaway session clone so the exported registry and history stay consistent. History is serialized in deterministic topological order, so identical delta sets export byte-identically.

## Resources

Everything content-addressable is a resource: raster images, fonts, embedded WASM, **and proto-node declarations**. The storage `Registry` holds `resources: ResourceStore` (references only). The bytes live in a content-addressed byte store keyed by `ResourceHash`, owned by the caller (the app-global cache in the editor, the `Gdd` container for standalone/export) rather than by `graph-storage`.

```rs
pub type ResourceStore = HashMap<ResourceId, ResourceEntry>;

pub struct ResourceEntry {
    pub sources: Vec<(SourceKey, SourceValue)>,     // fallback chain, sorted by key, LWW-element-set
    pub hash: Option<ResourceHash>,                  // resolved content hash (LWW)
    pub hash_timestamp: TimeStamp,
}

pub struct SourceKey   { pub priority: Priority, pub peer: PeerId }  // fractional priority + peer tiebreak
pub struct SourceValue { pub source: serde_json::Value, pub timestamp: TimeStamp }
```

A node references a resource by `ResourceId`. The entry maps it to a chain of `DataSource`s tried in order (`Embedded` bytes by hash, `FilePath`, `Url`, `Font`) plus the resolved `ResourceHash`. The chain is an **ordered LWW-element-set** keyed by `SourceKey`: each key carries a fractional `Priority` so a peer can insert between two sources without renumbering, and concurrent insertions at the same priority get distinct keys via the `PeerId` tiebreak. Distinct-key adds (the normal cross-peer case) therefore all survive. A same-key add versus remove resolves by LWW on the per-`Delta` timestamp rather than add-wins, so there are no tombstones, and causal delivery linearizes an add and its later removal. The `hash` is **LWW** (content-derived, so concurrent resolves agree by construction).

Each `DataSource` is stored as `serde_json::Value` rather than a typed enum, with the same motivation as the `Attributes` bucket: type-erasure lets migrations restructure variants without keeping old enum shapes alive. `DataSource` stays typed at the runtime layer, and conversion happens at the serialization boundary. Unknown variants are a hard error on load.

**Declarations as resources.** `Implementation::ProtoNode(ResourceId)` references a declaration resource. `from_runtime` serializes each `ProtoNode` through a self-describing `serde_json::Value` (MessagePack-encoded, via `encode_declaration`), hashes the bytes, derives the `ResourceId` from that hash, and registers a `DataSource::Embedded` entry, with the bytes going to the caller's byte store. (Deriving the ID from the hash is a deterministic bootstrap. A future stable well-known-ID table would let the ID denote the function.) `to_runtime` resolves declarations back via a `Declarations` map (`ResourceId` to `ProtoNode`) that the caller builds from its byte store. The self-describing form keeps `ProtoNode`'s serde aliases working so the on-disk shape stays migratable.

A `NodeInput::Value` stores its `TaggedValue` as a self-describing `serde_json::Value` (the same type-erasure as `Attributes` and `DataSource`), so the `TaggedValue` serde aliases keep working and the on-disk shape stays migratable. Legacy documents with inline image `TaggedValue`s have those values extracted into resources at load time, and new saves never embed inline image blobs in `NodeInput::Value`.

## Migrations

Migrations run on the type-erased `Registry`, after deserialization and before `to_runtime`. The pipeline reads the format version from the manifest, deserializes the registry with attributes as raw `serde_json::Value`, applies registered migrations scoped to the version range, and hands the result to `to_runtime`.

Migrations live in a dedicated crate so they are usable both from the editor and from a CLI for batch upgrades. A single global format version is used initially, and per-library versioning is a future extension.

# Reference-level explanation

## Conversion: runtime to storage

`from_runtime` flattens the recursive `NodeNetwork` into the flat `Registry`:

- Each node's path through the runtime nesting is hashed (blake3 truncated to 64 bits, with the document's `PeerId` mixed in) to produce a stable global `NodeId`. The original local ID is stashed in an attribute (`original_node_id`) so the round-trip can rebuild the runtime's per-network local IDs. Subsequent live edits mint fresh peer-scoped IDs via `Document::next_node_id` (`blake3(peer, counter)`) instead of going through the path-hash bootstrap.
- Each nested `NodeNetwork`'s `NetworkId` is derived from the owning node's path (blake3 of `(peer, path)` with a `"network"` domain tag), not assigned by a traversal counter. This makes it stable across a `to_runtime` then `from_runtime` round trip. That stability is load-bearing, because node paths (and thus node-ID hashes) include `NetworkId`s, so an unstable network ID would cascade into unstable node IDs and break re-commit after open. Aliasing (multiple nodes referencing the same network) is structurally supported by the storage model, since `Implementation::Network(NetworkId)` is a reference, but the converter does not exploit it yet. Aliasing is fixed at the runtime layer first, and the converter then preserves sharing without an explicit dedup pass.
- Non-structural `DocumentNode` fields (`call_argument`, `context_features`, `visible`, `skip_deduplication`, and so on) become entries in the node's `attributes`. UI metadata from `DocumentNodeMetadata` (positions, display names, locked, pinned, and so on) flows through the same bucket under `ui::*` keys.

`to_runtime` is the inverse. It rebuilds local IDs from the stashed attribute, restores typed fields from attribute values, follows `Implementation::Network` references to recursively materialize nested networks, and resolves `Implementation::ProtoNode(ResourceId)` against a `Declarations` map (`ResourceId` to `ProtoNode`) the caller supplies from its byte store. Since `graph-storage` is byte-unaware, `to_runtime` takes the resolved declarations as a parameter rather than reaching for bytes itself.

## Slots: inputs and exports

`Vec<InputSlot>` and `Vec<ExportSlot>` are positionally indexed at the storage layer. Each slot carries its own `TimeStamp`, giving LWW per slot on concurrent edits.

`ExportSlot` is sparse: `target == None` means the slot has been removed. `InputSlot` is dense. The runtime conversion compacts exports into a dense `Vec<NodeInput>` (preserving the runtime's "remove an export shifts later positions" semantics) and strips input timestamps.

Because inputs are stamped, `NodeInput::Node` references are set directly via `ChangeNodeInput`, with no add/remove rewire workaround.

## CmRDT semantics

- **Timestamps.** `TimeStamp { counter: u64, peer: PeerId }` is a Lamport counter with a peer-ID tiebreak. Comparison is lexicographic (counter first, then peer). Wall-clock time is not used.
- **NodeId identity.** Every new `AddNode` issues a peer-scoped ID, so concurrent creates cannot collide.
- **Causal delivery.** `apply_delta` requires that every parent of the delta (its `parent` plus any `Merge` extra parents) is already in local history. The storage layer does not buffer, and out-of-order delivery is a transport concern. New peers initialize via snapshot transfer (`Registry` plus history) before streaming deltas.
- **Removal.** Physical, with no tombstones. If a later op targets an absent node or network, the receiver searches its ancestry (from `head`, following all parents) for the delta that removed the entity, the one whose `reverse` is the matching `AddNode` or `AddNetwork`, and re-applies that reverse before applying the incoming op. `RemoveNode` and `RemoveNetwork` each carry a `snapshot` of the removed entity inside that reverse so the rebuild is O(1) and needs no further history walk. The snapshot is required because retirement recomputes an op's reverse *after* the hot op already applied the removal, when the live entity is gone. Removal is therefore non-durable under concurrent edits, since any concurrent reference to a removed node revives it.
- **LWW primitives.** Per-input (`InputSlot.timestamp`), per-export-slot (`ExportSlot.timestamp`), and per-attribute-value (the `TimeStamp` in `Attributes`). The exported-nodes list is one such attribute value (the `exported_nodes` document attribute), so it inherits per-key LWW with no separate machinery. The timestamp driving every LWW arm comes from the wrapping `Delta`. `AttributeDelta` carries `value: Option<_>` so a single shape covers both `Set` (`Some`) and `Remove` (`None`), and `Set` versus `Remove` has a defined winner.
- **Resources.** A resource's `hash` is LWW (content-derived, so concurrent resolves agree). Its source chain is an ordered LWW-element-set keyed by `SourceKey` (fractional priority plus peer tiebreak): concurrent `AddSource`s at distinct keys all survive, and a re-add or a remove at the same key is LWW on the per-`Delta` timestamp (no tombstones, so a same-key add/remove is order-sensitive only without causal delivery). Whole-resource `AddResource`/`RemoveResource` mirror the node/network add-remove pairs (`RemoveResource` snapshots the entry for O(1) reverse).

The CRDT does not mask graph-shape conflicts. Concurrent same-slot `SetNetworkExport`s with different targets resolve by LWW, but the resulting wiring may be wrong, and downstream consumers see it as a compile or wiring error.

## History storage

`History` is a `Vec<Delta>` in topological order (parents before children) with a `HashMap<Rev, usize>` index for lookup. The append order is canonical, which is what lets history serialize byte-identically across peers that absorbed the same delta set. `Document` adds a `head: Option<Rev>` (the local cursor, which advances only on local commits, `None` until the first commit) and a `hot_log: Vec<HotOp>` (in-flight unretired ops). Walking history follows each delta's `parent`, and this default first-parent walk reconstructs a single peer's local chain. Branches are siblings under a shared parent, and a `Merge` delta rejoins them, its extra parents naming the other tips it folds in.

## Editor metadata

`DocumentNodePersistentMetadata` and `NodeNetworkPersistentMetadata` from the runtime flow through the storage `Attributes` bucket under `ui::*` keys. That covers display names, locked/pinned flags, navigation/PTZ state, selection undo/redo stacks, and layer/node type metadata. Transient runtime caches (`DocumentNodeTransientMetadata`, click targets, resolved types, `OriginalLocation`) stay runtime-only and are not stored.

Document-scoped editor settings (viewport view, render mode, overlay/ruler visibility, snapping, collapsed layers) ride the document-level `Registry.attributes` under `ui::*` keys (`ui::ptz`, `ui::render_mode`, and so on). This is what makes the `.gdd` a lossless replacement for the legacy format's document-handler fields.


# Drawbacks

- **Diffing two full `Registry`s on every autosave is O(N) in document size.** This is the interim cost of treating storage as a serialization layer derived from the runtime. It is currently triggered at autosave boundaries (`commit_storage_snapshot`) rather than per gesture, and addressed long-term by computing deltas directly on runtime mutations.
- **Attributes as `serde_json::Value` carry per-value overhead.** This is mitigable with a typed fast path for hot keys without changing the design. They also force a self-describing codec, ruling out the most compact binary formats.
- **Single global format version is a sharp edge** when libraries diverge: a breaking change in one library bumps the version for documents that do not use it.
- **`RemoveNode` is non-durable under concurrency.** Any concurrent reference to a removed node revives it from history.

# Rationale and alternatives

**Delta-based vs. cleaner snapshot format.** A delta is the right unit for history, CRDT sync, and incremental compilation. Picking one representation for all three eliminates conversion seams between subsystems that need to interoperate.

**CmRDT vs. state-based CRDT or OT.** State-based CRDTs require a merge function and large state vectors. OT requires a central server to mediate transforms. CmRDT only requires per-op commutativity plus a causal-order delivery layer, which the transport provides.

**Merkle `Rev` (parent in the hash) vs. a position-independent content id.** A `Rev` hashes `(parent, author, timestamp, kind)`, and since `parent` is itself a `Rev`, every `Rev` transitively commits to its whole ancestry. This is the Git commit-hash model: a Git commit folds its parent SHA, tree, author, committer, and message into its own SHA, so changing any ancestor rewrites every descendant hash. We keep that model for the same two reasons Git gets value from it. First, tamper-evidence: rehashing detects any rewrite of history (see [Deltas](#deltas)). Second, the chain doubles as the causal structure the CRDT already needs. We then diverge from Git in one deliberate place: a `Merge` is hashed over its sorted parent set alone, with author and timestamp excluded, so two peers merging the same tips mint the *identical* `Rev` and it dedups into one shared node. Git does the opposite (its merge commits carry author/time/message and so never converge), because Git reconciles through a human pushing and pulling rather than through automatic DAG convergence. The cost of parent-in-hash is the one Git also pays: reordering or rebasing an op rewrites every descendant `Rev`, so identity is not stable across [history linearization](#future-possibilities). A pure content-derived id (no parent) would survive reordering but would forfeit both convergence-by-construction and free tamper-evidence, so it is the wrong primary identity. The position-independent use case is better served by a separate, computed id (see the patch-id analogue under [Future possibilities](#future-possibilities)) rather than by weakening `Rev`.

**Ad-hoc resurrection vs. tombstones.** Tombstones add a permanent footprint to the data model and a GC policy question. Resurrection reuses the history log already needed for undo as the recovery mechanism, keeping the live `Registry` lean. The cost is that `RemoveNode` is not durable under concurrent edits.

**Type-erased attributes vs. typed metadata fields.** Migrations operate on attribute values without keeping old Rust struct shapes alive. The cost is per-value overhead, mitigable without changing the model.

**Flat node storage vs. nested networks.** All CRDT ops target nodes via a single uniform `NodeId` address space regardless of nesting depth. A nested representation would require ops to carry a path, complicating commutativity.

**`.gdd` vs. reusing `.graphite`.** A distinct extension makes migration unambiguous and prevents older Graphite versions from trying to open a new-format file.

**One self-describing binary codec (MessagePack).** The persisted bodies (the registry, the history of deltas, and the `ProtoNode` declaration resources) serialize as their typed Rust shapes, but each carries type-erased `serde_json::Value` leaves: attribute values, `NodeInput::Value` payloads, and resource source bodies. Those leaves need a self-describing codec to deserialize and to keep the serde-alias migration path alive, which forces the same requirement on the whole payload. MessagePack provides that at a few percent size cost. Hash preimages (`NodeId`, `Rev`, node-path hashes) use the same codec: a fixed serializer emits one deterministic byte form per value, which is all `blake3` needs, so the codec doubles as the canonical hash encoding without a second format.

**Source chain as a sorted `Vec` vs. `BTreeMap`.** `SourceKey` is a struct, so a `BTreeMap`-keyed chain cannot serialize to JSON (string keys only). A sorted `Vec` of pairs keeps the same ordering and per-key LWW semantics losslessly across every codec.

# Future possibilities

- **Per-library format versioning** so a breaking change in one library doesn't bump the version for documents that don't use it.
- **History linearization.** Prune unused branches from a convoluted tree to produce a clean undo/redo history.
- **A patch-id analogue for position-independent identity.** A `Rev` rewrites under reordering because it commits to its parent, so it cannot answer "is this the same logical edit as that one, somewhere else in history?" Git solves the same problem with `git patch-id`, a hash of the normalized *diff* that is independent of parent and commit metadata. The analogue here is a `content_id` hashed over the op payload with the bookkeeping fields (parent, author, Lamport timestamp) normalized out. Crucially it need not enter the data model: it is derivable from a `Delta` on demand, so it can be computed when linearization or cross-document dedup needs it without adding a stored field or touching the format version. This keeps `Rev` as the sole stored identity while still enabling identity-preserving linearization and recognizing a shared edit across documents.
- **Runtime-native deltas.** Move delta computation out of the storage layer into the runtime, eliminating per-edit `Registry` re-conversion.
- **Incremental compilation driven by deltas.** The compiler consumes runtime deltas and recompiles only changed regions.
- **Runtime-level aliasing for shared node-network definitions.** Storage already supports `Implementation::Network` as a reference, and once the runtime supports sharing natively, the converter preserves it.
- **Online migration service.** Active editors drop migrations older than some threshold, and old documents go through a remote upgrade pipeline first.
- **Distributed / signed history.** Content-addressed `Rev` plus signing enables multi-author provenance and verifiable history.
- **Libraries as files.** A follow-up RFC will specify how `.gdd` files act as importable libraries via the document's `exported_nodes` list.
