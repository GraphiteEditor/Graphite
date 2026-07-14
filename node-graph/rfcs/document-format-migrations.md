# Summary

A migration system for `.gdd` documents, split into a trait crate (`migration-core`), self-contained migration crates, and a dispatching runner crate (`migration-runner`). Migrations come in two tiers: **format migrations** step a document's serialized payloads from one format version to the next before typed deserialization, and **content migrations** upgrade node usages on the typed `Registry` within the current format version, committed to history as ordinary deltas. Minimal dependencies per migration crate keep historic migrations cheap to build and store, and open the path to shipping them as wasm modules for an online migration service.

# Motivation

The legacy `.graphite` machinery mixes three mechanisms (string preprocessing, serde aliases, post-deserialize fixups), all of which keep old runtime shapes alive in the active codebase (see the Migrations section of [document-format.md](document-format.md)). Surveying it yields the operation catalog a replacement must cover:

- Identifier renames and alias tables (~130 proto-node remaps in `NODE_REPLACEMENTS`).
- Node shape changes: add/drop/permute inputs, staged multi-version upgrades (Morph v1→v2→v3), where the input count acts as an implicit version.
- Value transforms with graph fallout: unit conversions on literal inputs, and conversion-node splices when the input is wired instead.
- Structural rewrites: node splits that preserve the original `NodeId` for reference stability (Blending → Blend Mode/Opacity/Clip), wrapper-network collapses (Brush/Transform/Image), catalog-default resets.
- Data externalization: inline image values extracted into content-addressed resources.
- Metadata normalization: `call_argument` upgrades, layout repair around inserted nodes.

The new format adds a class the legacy system never had: stored history. Deltas embed node shapes (`AddNode`, removal snapshots), so a shape change must either rewrite stored deltas, which rehashes the Merkle `Rev` chain, or truncate history.

# Guide-level explanation

## Two tiers

**Format migrations** are whole-document version steps: exactly one per `format_version` bump, keyed `migrates_from → migrates_from + 1`. They run before typed deserialization, on serialized payloads (`Payload` = bytes + codec). A format-migration crate freezes whatever old struct shapes it needs *locally*, deserializing payloads into its own mirror types; the active codebase never carries them. Retiring the migration (for example, to the online service) removes the frozen shapes from the repo. The "no old shapes" goal only ever applied to the runtime crates.

**Content migrations** upgrade node usages within the current format version. They run after deserialization into the typed `Registry`, before `to_runtime`. Selectors:

- `Node(DeclarationMatch)` — every node whose proto-node declaration matches by identifier (with historic aliases) or by declaration content hash. Declarations are content-addressed resources, so a hash pins an exact node version, replacing the legacy input-count sniffing.
- `Reference(name)` — every node carrying a given `ui::reference` attribute (how legacy wrapper networks like "Brush" are identified).
- `Document` — once per document, for bulk normalization passes.

## Delta-expressed content migrations

A content migration mutates a clone of the working registry with plain Rust. The runner diffs the clone against the working registry (`compute_deltas`), stages the difference as ordinary hot ops, and retires them as one gesture authored by the migrating peer. The upgrade is therefore recorded in history, undoable, and converges across peers like any other edit. Migrations never construct deltas by hand, so they cannot emit malformed histories.

Timestamps inside the mutated clone are irrelevant: the diff is value-only and the commit path stamps every emitted op with fresh clock ticks, so migrations set attribute values with any placeholder timestamp.

## History under format migrations

Each format migration declares a `HistoryPolicy`:

- `Untouched` — the shape change does not affect stored deltas.
- `Rewrite` — the migration transforms each delta record's payload; the runner then recomputes `Rev`s in topological order and remaps parent links and session cursors (`head`, redo stack, `last_broadcast_rev`). Because the rewrite is a pure function of content and `Rev`s are content-addressed, peers applying the same migration converge on identical rewritten history without coordination, the same dedup-by-construction argument `Merge` relies on.
- `Truncate` — no faithful rewrite exists; the document becomes a state-only snapshot (the `include_history: false` export shape).

Migrations never touch identity fields (`id`, `parent`); Merkle bookkeeping belongs entirely to the runner.

## Crate layout

```
document/migrations/
├── core/     migration-core: traits, selectors, Payload, errors
├── runner/   migration-runner: version chaining, target scanning, delta commit, Rev rehash
└── <sets>    one crate per migration era or version step, exporting `fn migrations() -> MigrationSet`
```

`migration-core`'s mandatory dependencies are `serde`, `serde_json`, `rmp-serde`, and `thiserror`. The `typed` feature (default) adds `graph-storage` (no default features, itself dependency-light) for the content tier; format-tier-only migration crates build without `graph-storage` entirely. Registration is explicit: each set crate exports a plain constructor and the runner aggregates. No linker-section registry (`inventory`/`linkme`), keeping wasm compilation trivial.

# Reference-level explanation

## Runner pipeline

1. Read `manifest.format_version`. While it is below the target version, apply the format migration whose `migrates_from` matches (a gap is a hard error), transforming registry/history/session payloads per its policy, and bump the version.
2. Deserialize the now-current payloads into typed `graph-storage` structures. If any step declared `Rewrite`, rehash the delta DAG and remap session cursors.
3. For each content migration in registration order: scan the registry for selector matches, clone the working registry, apply the migration per target (reverting a target's changes if it errors, so one bad node doesn't poison the rest), diff, and commit as a migration gesture.
4. Hand off to `to_runtime`.

Erroring migrations are logged and skipped rather than failing the load, matching the legacy behavior of preferring a partially-upgraded document over no document.

## Provenance and idempotency

Node-selector migrations are naturally self-gating: once the declaration is rewritten, the selector no longer matches. `Document`-selector migrations are guarded by a provenance list under the `migrations::applied` document attribute, which the runner appends to after a successful run (through the same diffed commit, so provenance rides history too). Content migrations must still be written idempotently, since a document may round-trip through an editor build that lacks a later migration.

## Host services

Content migrations reach the outside world only through a `MigrationContext` trait implemented by the host (editor or CLI): minting peer-scoped IDs, resolving declaration resources to identifiers, and instantiating current catalog defaults. This keeps migration crates independent of the editor.

## Wasm trajectory

The format-tier boundary is already bytes-in/bytes-out per payload, so a format migration crate compiles to a wasm module with a small shim and no host callbacks. Content migrations need the typed `Registry` and a `MigrationContext`, so wasm-shipping them means serializing the registry across the boundary and defining a small host-function surface; that design is deferred until the online migration service is scoped.

# Rationale and alternatives

**Frozen shapes in migration crates vs. type-erased everything.** An earlier sketch had all migrations operate on `serde_json::Value`. That fails on the actual payloads: `Rev` is a bare `u128` and MessagePack map keys are integers, neither representable in `serde_json::Value`. Typed frozen mirrors sidestep both, and they live in removable migration crates rather than the runtime.

**Diff-based delta expression vs. hand-written deltas.** Reusing `compute_deltas` means content migrations are ordinary Rust mutations. The cost is a registry clone per migration (plus one per target for error isolation), acceptable at load time.

**Explicit registration vs. `inventory`.** Linker-section registries complicate wasm and cross-crate builds for zero gain at this scale.

# Future possibilities

- CLI `migrate` subcommand for batch upgrades (the runner is already editor-independent).
- Online migration service running retired migration crates as wasm modules, letting active editors drop ancient migrations.
- Declarative rule data (alias tables, input permutations) shippable without code, once the imperative patterns stabilize.
- Per-library format versioning, as in the format RFC.
