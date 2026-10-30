# Summary

A migration system for `.gdd` documents built around one rule: **history is always in the current shape**. A migration is a deterministic function over registry states. The runner applies it by replaying stored history and rewriting every delta into what it would have been had the author worked against the new shape, inserting deltas where the migration restructures the graph. Undo, time travel, replay, and deltas arriving from peers on an older build therefore never meet an outdated node shape, and the runtime never needs to execute one.

Migrations come in two tiers sharing that rule. **Format migrations** step serialized payloads across a `format_version` bump. **Node migrations** step one proto-node declaration from `identifier@version` to its successor on the typed `Registry`. Migrations are written as plain functions whose parameter types state what they read and write, live in independent crates that register themselves, and are ordered automatically.

# Motivation

The legacy `.graphite` migrations (`editor/src/messages/portfolio/document_migration.rs` plus serde shims across the runtime crates) are not a compatibility target. They are a record of what goes wrong, and each lesson below became a principle of this design.

| Legacy problem | Principle |
|---|---|
| Versions inferred from input counts and value variants; a wired input defeats value sniffing. | Explicit versions. Every change to a declaration's persisted shape or behavior bumps its version, and a migration is exactly one step. |
| Migrations build their result from the *current* catalog (`default_node_template()`), so an old migration's output drifts as the catalog evolves. | Frozen migrations. A migration spells out every node and value it produces. |
| Migration code calls editor business logic (`replace_inputs` with layout side effects, `insert_node_between`, `set_stroke_paint_order`). | Isolation. Migrations see plain registry data through views owned by `migration-core` and depend on no editor or runtime crate. |
| Runtime crates keep old shapes alive for migrations (`LegacyFill`, `XY`, serde aliases, untagged fallbacks, `serde(default)` fields added without a version bump). | No compatibility code in runtime types. Old value shapes live only in migration crates as frozen mirrors. |
| One long function with order coupling, stale snapshots, and early returns that silently skip later upgrades. | One concern per migration, ordered by explicit dependencies, reading an immutable source state. |
| Inserted nodes and resources get random IDs. | Determinism. Every ID a migration creates derives from the migration, the anchor node, and the inserted declaration. |
| Silent loss and heuristics ("first consumer only", dropping wired values, skipping nodes that hold default values). | Totality. A migration is defined on every state its source version can be in, or it fails explicitly. |
| About 170 renames in an ever-growing alias table, applied by substring and prefix matching. | Renames are ordinary one-line migrations, detected by CI and ordered like any other. |
| Each document open re-reconciles node instances against the catalog (`validate_input_metadata`, `normalize_stored_types`) because instances duplicate declaration data. | Instances don't store what their declaration defines. |
| Migrations committed as new edits leave old shapes in history, which undo resurrects and concurrent edits corrupt (see below). | History rewrite with translation. |
| Most migration blocks have no test, and nothing notices a shape change that ships without a migration. | A CI check compares node APIs against the baseline, and every migration passes the commutation property test. |

## Why migrations can't simply be new deltas

Committing a migration as an ordinary gesture on top of history keeps old shapes reachable, and it corrupts concurrent edits. Suppose version 1 of a node has inputs `[a, b]` and version 2 inserts a slot at 0, giving `[new, a, b]`. Peer A migrates. Concurrently, peer B, not yet migrated, writes `ChangeNodeInput { input_idx: 1 }` to change `b`. After merging, B's write lands on slot 1 of the version 2 node, which is `a`. B's delta has to be *translated* into the new shape, and once deltas are translated, rewriting all of history is the same machinery applied uniformly.

# Guide-level explanation

## Versioned declarations

A proto-node declaration (`graph-storage::ProtoNode`) carries its `identifier` and a `version`. The identifier stays inferred from the module path as today, and the version is declared on the node, defaulting to 1:

```rust
#[node_macro::node(category("Blending"), icon("NodeOpacity"), version(2))]
fn opacity<T>(/* ... */) -> Item<T> {
	// ...
}
```

The macro records the version in `NODE_METADATA` as `NodeMetadata::version`. A declaration is identified by `identifier@version`. Declaration resources are content-addressed from that pair, so the same declaration has the same `ResourceId` on every peer, and a document states the exact version of every node it contains.

A node's **lineage** is the chain of declarations connected by migrations. Every migration bumps the version by one, including renames, so `identifier@version` never repeats within a lineage even if a node is renamed back and forth (`a@1 → b@2 → a@3`). The version declared on a node is its position in its lineage. It is declared rather than derived because a binary shipping only a subset of migration crates can't count the lineage.

## When a migration is required

A CI check (see "API change detection") compares every node's public API against the baseline branch:

- An identifier disappeared. A migration from `old@version` is required. When a new identifier with an identical API appeared, the check suggests a rename.
- The API changed (inputs, their order or types, the output type) without a version bump. The check asks for a bump.
- The version was bumped. A migration from the previous version is required.

Behavior changes that keep the API are invisible to the check. Their author bumps the version manually, like a semver bump for a behavior change.

## Writing node migrations

A node migration is a function annotated with `#[migration]`, naming its source and target declarations:

```rust
#[migration("blending_nodes::opacity"@1 => @2)]                                // Version step
#[migration("blending_nodes::blending"@1 => "blending_nodes::blend_mode"@2)]   // Rename or replacement
```

The function runs once per **anchor**: a node of the source declaration. The runner switches the anchor to the target declaration itself. What the function may read and write is stated entirely by its parameter types:

- **Shared references read the source state.** `&Node` is the anchor before migration, and `&Graph` is its network before migration. Reads never observe the migration's own edits.
- **Mutable references are edit views.** Each owns a region of the registry and records edits there. The macro rejects two mutable views with overlapping regions at compile time.

| Parameter | Region | Operations |
|---|---|---|
| `&Node` | anchor (read) | the existing `Node` accessors, plus `id()`, which the stored node doesn't hold |
| `&Graph` | anchor's network (read) | `declaration(node)`, `node(node)`, `consumers(output)` |
| `&mut Inputs` | the anchor's input slots | `set`, `insert`, `push`, `remove`, `truncate` |
| `&mut Upstream` | new and existing nodes upstream of the anchor | `add_node`, `add_node_keyed`, `remove_node` |
| `&mut Downstream` | the slots and exports reading the anchor's outputs, plus new downstream nodes | `consumers`, `add_node`, `add_node_keyed`, `connect` |
| `&mut Resources` | document resources | `add_embedded` |

`Inputs` operations apply in order to the target layout being built. `node.inputs()` is always the source layout. Reads through a mutable view (such as `Downstream::consumers`) also see the source state, so nodes added by the migration never appear among the anchor's consumers.

A migration addresses declarations with `decl!`, values with a frozen mirror of the value variants it uses, and connections with plain connector types:

```rust
/// A declaration at one version.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Declaration {
	pub identifier: &'static str,
	pub version: u32,
}

#[macro_export]
macro_rules! decl {
	($identifier:literal @ $version:literal) => {
		$crate::Declaration::new($identifier, $version)
	};
}

#[derive(Clone, Copy)]
pub struct OutputConnector {
	pub node: NodeId,
	pub output: u32,
}

pub enum InputConnector {
	Node { node: NodeId, input: u32 },
	Export { network: NetworkId, index: u32 },
}

/// A node to add, with every input given explicitly.
pub struct NewNode {
	pub declaration: Declaration,
	pub inputs: Vec<NodeInput>,
}
```

Input values are stored as self-describing `graph-storage::Value`s. Each migration crate declares a frozen mirror of the value variants it reads or writes, and converts with `encode(value) -> Value`, `literal(value) -> NodeInput` (unexposed), and `literal_of::<T>(&NodeInput) -> Option<T>`, which is `None` for wires and other variants:

```rust
/// Frozen mirror of the `TaggedValue` variants this crate's migrations use.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Tagged {
	Number(f64),
	Integer(i64),
	Bool(bool),
	Resource(ResourceId),
}
```

## Examples

A rename needs no function:

```rust
rename!("graphene_core::ops::AddNode"@1 => "math_nodes::add"@2);
```

Adding inputs only touches the anchor's slots:

```rust
/// Adds the opacity and fill toggles.
#[migration("blending_nodes::opacity"@1 => @2)]
fn opacity_toggles(inputs: &mut Inputs) {
	inputs.insert(1, literal(Tagged::Bool(true)));
	inputs.push(literal(Tagged::Bool(false)));
	inputs.push(literal(Tagged::Number(100.)));
}
```

A value transform must handle a wired input as well as a literal one:

```rust
const ROTATION: u32 = 2;

/// Rotation is now stored in degrees.
#[migration("transform_nodes::transform"@1 => @2)]
fn transform_degrees(node: &Node, inputs: &mut Inputs, upstream: &mut Upstream) -> Result<(), MigrationError> {
	let rotation = &node.inputs()[ROTATION as usize].input;

	let converted = match rotation {
		NodeInput::Value { value, exposed } => {
			let Some(Tagged::Number(radians)) = literal_of(rotation) else { return Err(MigrationError::invariant("rotation is not a number")) };
			NodeInput::Value { value: encode(&Tagged::Number(radians.to_degrees())), exposed: *exposed }
		}
		// Only known at runtime, so convert upstream
		wired => {
			let multiply = upstream.add_node(NewNode {
				declaration: decl!("math_nodes::multiply"@1),
				inputs: vec![wired.clone(), literal(Tagged::Number(180. / PI))],
			});
			NodeInput::Node { id: multiply, index: 0 }
		}
	};

	inputs.set(ROTATION, converted);
	Ok(())
}
```

A change in output meaning rewires the consumers:

```rust
/// The output is now in degrees, so consumers keep receiving radians through a conversion.
#[migration("vector_nodes::angle_between"@1 => @2)]
fn angle_between_degrees(node: &Node, downstream: &mut Downstream) {
	let consumers = downstream.consumers(0);
	if consumers.is_empty() {
		return;
	}

	let conversion = downstream.add_node(NewNode {
		declaration: decl!("math_nodes::to_radians"@1),
		inputs: vec![NodeInput::Node { id: node.id(), index: 0 }],
	});
	for consumer in consumers {
		downstream.connect(OutputConnector { node: conversion, output: 0 }, consumer);
	}
}
```

A replacement that changes the output layout, with a selector that may be wired:

```rust
/// Extract XY becomes Split Vec2 with one output per component. A wired axis selects between them at runtime.
#[migration("vector_nodes::extract_xy"@1 => "vector_nodes::split_vec2"@2)]
fn extract_xy_to_split(node: &Node, inputs: &mut Inputs, downstream: &mut Downstream) -> Result<(), MigrationError> {
	let axis = &node.inputs()[1].input;

	let component = match literal_of::<Axis>(axis) {
		Some(Axis::X) => OutputConnector { node: node.id(), output: 0 },
		Some(Axis::Y) => OutputConnector { node: node.id(), output: 1 },
		None if matches!(axis, NodeInput::Value { .. }) => return Err(MigrationError::invariant("axis is not an XY value")),
		None => {
			let select = downstream.add_node(NewNode {
				declaration: decl!("logic_nodes::select_axis"@1),
				inputs: vec![axis.clone(), NodeInput::Node { id: node.id(), index: 0 }, NodeInput::Node { id: node.id(), index: 1 }],
			});
			OutputConnector { node: select, output: 0 }
		}
	};

	// Source state only, so the select node added above isn't among these
	for consumer in downstream.consumers(0) {
		downstream.connect(component, consumer);
	}
	inputs.truncate(1);
	Ok(())
}
```

## Rewriting history

The runner replays history in canonical order. For each delta `d` it holds two states, both determined by `d`'s causal past only:

- `O(d)`: the **original** state, the replay of `d`'s ancestors as authored. This is what the author saw.
- `R(d)`: the **rewritten** state, the replay of the already rewritten ancestors.

Applying a migration to every anchor in a state defines a total function `M` over registry states. The rewritten form of `d` is the difference between where the rewritten history stands and where it should stand:

```
rewritten(d) = diff( R(d), M(O(d) + d) )
```

The difference is staged against `R(d)` through the ordinary op path, which produces correct reverse priors, and replaces `d` in history. In the common case it is a single delta: a version step becomes `SetNodeImplementation` to the target declaration, and an input write is moved to its new slot. When the migration restructures the graph it becomes a short chain, for example `AddNode` for an inserted node followed by the rewires. A delta whose difference is empty is dropped, and its interaction-end mark moves to the rewritten delta before it. An interaction whose effect the migration makes empty therefore stops being its own undo step.

Translation of later deltas falls out of the same formula. If a migration inserts a conversion between `X` and its consumer `Y`, and later in history the user rewires `Y.0 = X`, then `M` applied to that original state again routes `Y` through the conversion, so the rewritten delta wires `Y` to it. Deltas that never touch a source declaration's node pass through unchanged, because `M` is the identity away from anchors.

## Two tiers

**Format migrations** step the serialized payloads (registry, history records, session) from `format_version` `n` to `n + 1`. They decode with the frozen types of version `n` and encode with those of `n + 1` (see "Frozen format versions"), never with the live types, and their history rewrite is per record. A format change that cannot be expressed per record uses `Truncate` (see below). History-only data that no state captures (annotations, authorship) is also format-tier territory.

**Node migrations** run after the document is deserialized at the current format version, one history rewrite pass per migration, in dependency order.

Both tiers end in the same place: stored history, the registry snapshot, and session cursors all in the current shape, with `Rev`s recomputed.

# Reference-level explanation

## What `#[migration]` expands to

Every parameter type implements a view trait:

```rust
pub trait View<'a>: Sized {
	const REGION: Region;
	fn extract(context: &AnchorContext<'a>) -> Self;
	fn into_edits(self) -> Result<Edits, MigrationError>;
}
```

`into_edits` fails on a misuse only visible at runtime, such as an input index out of range or a `connect` to a slot that doesn't read the anchor.

The attribute generates a `NodeMigration` implementation that extracts each mutable view, calls the function with the source state and the views, and collects their edits. For `angle_between_degrees`:

```rust
struct AngleBetweenDegrees;

impl NodeMigration for AngleBetweenDegrees {
	fn source(&self) -> Declaration {
		decl!("vector_nodes::angle_between"@1)
	}
	fn target(&self) -> Declaration {
		decl!("vector_nodes::angle_between"@2)
	}
	fn pins(&self) -> &'static [Declaration] {
		const { &[Declaration::new("math_nodes::to_radians", 1)] }
	}
	fn footprint(&self) -> Region {
		Region::ANCHOR.union(<Downstream as View>::REGION)
	}

	fn run(&self, context: &AnchorContext) -> Result<Edits, MigrationError> {
		let mut view = <Downstream as View>::extract(context);
		angle_between_degrees(context.source_node(), &mut view);
		View::into_edits(view)
	}
}
```

The struct is named after the function in Pascal case and carries its doc comment. A migration with several mutable views also gets `const _: () = assert!(Region::disjoint(&[...]))` over their regions, so overlapping views fail to compile. The macro rejects a step that doesn't advance the version by exactly one, and parameters other than `&Node`, `&Graph`, and `&mut` views. Pins need the inline `const` block, since a call to a `const fn` isn't promoted to `'static`.

It also registers the migration (see "Registration"). The runner applies the `Edits` to a registry clone, switches the anchor to the target declaration, and diffs.

## Deterministic IDs

A node added by a migration gets the ID `hash(migration, anchor, declaration, key)`. The key defaults to empty, so `add_node` needs no name as long as a migration adds at most one node per declaration for an anchor. `add_node_keyed` disambiguates several nodes of one declaration, and a duplicate `(declaration, key)` within one run is an error.

The ID must be the same in every state the migration runs on, because the rewrite evaluates `M` once per relevant delta. Only then does the diff between two states see "same node, different input" rather than a removal plus an addition, and later deltas that wire to the inserted node keep pointing at it. A call counter would break as soon as an insert is conditional, and hashing the inserted node's inputs would change the ID whenever an input changes. The declaration is constant per migration (see "Ordering"), so including it is as stable as the key.

Embedded resources are content-addressed from their bytes, so a migration that encodes data must do so deterministically. The migration crate pins its encoder's version.

## Ordering

Migrations are ordered by their declarations, not by registration:

- **Chain:** a migration whose target is `a@n` runs before the one whose source is `a@n`.
- **Pins:** every `decl!` in a migration's body pins the declaration it names. A migration that inserts or inspects `fill@2` runs after the migration that produces `fill@2` and before `fill@2 → 3`, so the nodes it creates or sees always have the version it was written against. The attribute macro collects the pins by scanning the body for `decl!`.

Cycles are rejected in CI, as are two migrations from the same declaration. Migrations with no ordering relation commute, because their footprints contain different anchors. The runner still breaks ties by source declaration, so the order never depends on registration order.

## Detecting pending migrations

A node migration is pending when its source declaration is referenced anywhere in the document, *including history*. A node removed long ago still has its old shape stored in its `AddNode` and in the removal snapshot, and undo can bring it back. Pending format migrations come from `manifest.format_version`.

## Runner pipeline

1. **Format tier.** While `format_version` is below the current version, apply the step whose `migrates_from` matches (a gap is a hard error) to the registry, history records, and session, then bump the version.
2. **Deserialize** into typed `graph-storage` structures.
3. **Node tier.** For each pending node migration in dependency order, rewrite history:
   1. Walk deltas in canonical order, keeping `O` and `R` for the causal past of each delta.
   2. Compute `rewritten(d)`, stage it against `R(d)`, and record the mapping from `Rev(d)` to the `Rev` of the last delta of its rewritten chain. A dropped delta maps to the rewritten `Rev` of its parent.
   3. Rewrite parent links (including `Merge` extra parents), annotations such as interaction-end marks, and session cursors (`head`, redo stack, `last_broadcast_rev`) through the mapping.
   4. Settle the tips (see "Merges and settling").
4. Persist the rewritten history, registry snapshot, session, and the `Rev` mapping.
5. Hand off to `to_runtime`.

`Gdd::open_in` runs this pipeline when `document-format` is built with its `migrations` feature, using the steps and migrations the binary links. Hot ops retire into history before the node tier, so they migrate with it. The resource a migrated node references is the content-addressed `ProtoNode { identifier, version }` with no attributes, the same one runtime conversion stores for a node at that version, so the runner registers it and stores its bytes without the target binary. A first version omits `version` from its encoding, which keeps declarations written before versions existed at their `ResourceId`. Format version 2 introduced the field, so older builds refuse documents that may contain later versions.

## Determinism and causal past

`rewritten(d)` depends only on `d` and its ancestors. The canonical order interleaves concurrent branches, so the linear replay state before `d` can contain deltas concurrent to `d`. Evaluating `M` there would make a peer that knows more history rewrite the shared prefix differently, and the `Rev`s would diverge. Restricting to the causal past makes the rewrite prefix-stable: learning more history never changes an already rewritten delta. Two peers that hold the same delta compute the same rewritten chain and the same `Rev`s, which is the same argument by which `Merge` dedups.

Every op of `rewritten(d)` must order after everything `d` saw and before everything that saw `d`, and two ops in one chain must not tie on the same slot. Lamport order already places every successor of `d = (c, p)` at a counter above `c`.

## Merges and settling

`M` is a function on states, not on concurrent edits, so the rewritten branches merged by the CRDT need not equal `M` applied to the merged original. For example, a migration that inserts a node only when the anchor has exactly one consumer gives different results when two branches each add one consumer.

The formula absorbs this at join points. For a `Merge` delta, `R(d)` is the CRDT merge of the rewritten branches and `O(d)` the merge of the original ones, so `rewritten(d)` carries exactly the fix-up that makes the merged state agree with `M`. Tips not yet joined by a `Merge` are settled by one final delta: `diff(replay(rewritten history), M(replay(original history)))`. When `M` composes with merging (the common case), the settle delta is empty.

## Peers on an older build

A delta from a peer that has not applied a migration references parents in the original `Rev` space. That is how the runner recognizes it as authored against the old shape, without any version field on the delta:

- Its parents resolve through the persisted `Rev` mapping, or are themselves deltas authored against the old shape.
- Its original state `O(d)` is reconstructed from the retained original history.
- It is translated with the same formula and merged.

The reverse direction is gated. A peer must not receive deltas referencing a declaration it doesn't know, so a session involving an older build either upgrades it or keeps it read-only. Because of this gate, the deltas authored against the old shape form an ancestor-closed region of the DAG: none has an ancestor authored against the new shape, which is what makes `O(d)` well defined.

## Efficiency

A naive rewrite evaluates `M` on a full state per delta. Instead, the runner:

- Keeps `O` and `R` incrementally, with checkpoints at branch points.
- Evaluates `M` only on anchors whose footprint intersects the nodes `d` writes, in either the original or the rewritten state. A migration's footprint is the union of its parameters' regions, so a `&Graph` read widens it to the whole network.
- Diffs only that region.

A migration whose edits are only `Inputs` operations with constant values, and every rename, is an index map over input slots. The runner applies such migrations per delta without replaying states, remapping `ChangeNodeInput`, `SetNodeInputs`, `ChangeNodeInputAttribute`, `AddNode` payloads, and removal snapshots directly. The commutation test proves this fast path equal to the state path.

## Errors and the truncate fallback

A migration reports two kinds of failure:

- `MigrationError::invariant`: the source state violates its own declaration's contract, such as a literal of the wrong variant in a non-exposable input.
- `MigrationError::unrepresentable`: the target version can't express a reachable source state. This is a design flaw in the new declaration, and the fix is to change the declaration (for example, keep an input), not to lose information.

Migrations are total by contract, so either error is a bug. A rewrite that failed on some deltas but not others would leave an inconsistent history, so the runner treats failure as all or nothing:

1. If the history rewrite fails, apply `M` to the final state only and **truncate**: history restarts from the migrated snapshot, and undo cannot cross it.
2. If `M` fails on the final state, open the document read-only and report the failure.

Format migrations whose change cannot be expressed per record declare `Truncate` up front. Truncation requires the collaborating peers to agree on the truncated head, so it is a fallback, not a default.

## Registration

Migrations register themselves the way nodes do. On native targets the generated registration runs through `#[ctor]`. On wasm, where `ctor` isn't available, the macro emits an exported `extern "C" fn __migration_registry_*` that the host calls on startup, alongside the existing node registry shims. The export's name hashes the step, so two crates registering the same step fail to link. Format steps register the same way through `format_migration!`, and the runner rejects two steps from one version. The runner reads the registry, orders it (see "Ordering"), and knows no specific migration. A rename is registered with `rename!` rather than a `migration!` form, because attribute and function-like macros share one namespace, and macro output must mint a unique exported symbol that `macro_rules!` can't.

Which migrations a binary ships is decided by which migration crates it links, selected with cargo features. A linked crate that nothing references can be discarded by the linker, so each binary references every enabled migration crate (`use nodes_2026 as _;` behind its feature).

## Frozen format versions

Every stored type lives in `graph-storage`'s `schema` module: the registry, deltas, hot ops, session state, and proto-node declarations, with the impls their encoding depends on. Behavior stays in the rest of the crate. `ResourceId` and `ResourceHash` stay in `graphene-resource`, whose encoding of them is fixed.

`cargo run migration snapshot` freezes the module into `migration-formats` as `v<N>`. A version declares again only the types whose definition or impls changed, plus every type reaching one, since `v<N>::Node` must refer to `v<N>::NodeInput`. Everything else is re-exported from `v<N - 1>`, so an unchanged type is the same Rust type in both versions and a format step converts only what changed. Comparison ignores docs, visibility, and formatting. A change to a leaf like `Value` still declares almost every type again, while a new `Delta` field declares few.

`cargo run migration check` fails in CI when the live module no longer matches `latest`, naming the changed types, or when `SUPPORTED_FORMAT_VERSION` isn't the latest frozen version. It also fails when a stored field names a type defined outside the module. A round-trip test encodes live documents through `latest` and compares the results, which catches what comparing definitions can't, such as a changed serde impl in a dependency.

Format steps live in `migration-formats/src/steps/`, depend only on its frozen versions, never on `graph-storage`, so the infrastructure can change without touching old migrations. `snapshot` writes the step along with the version and sets `SUPPORTED_FORMAT_VERSION`:

```rust
format_migration! {
	1 => 2,
	registry: v1::Registry,
	delta: v1::Delta,
	hot_op: v1::HotOp,
}

impl Upgrade for v1::ExportSlot {
	type Next = v2::ExportSlot;
	fn upgrade(self) -> Result<Self::Next, MigrationError> {
		let v1::ExportSlot { target, timestamp } = self;
		Ok(v2::ExportSlot {
			label: compile_error!("`label` is new; give it a value here"),
			target,
			timestamp,
		})
	}
}
```

`format_migration!` registers a step converting each listed root (the ones the version declares again) through its `Upgrade` impl, and leaves the other payloads untouched. Listing `delta` sets `HistoryPolicy::Rewrite`. The tool generates an `Upgrade` impl for every type declared again: types that only reach a changed one convert field by field, mapping containers structurally, with aliases expanded and generic types like `Tombstone<T>` converted inline at each use, since an impl on a type shared across versions would conflict between steps. A changed type converts the fields and variants that still match by name and type, and marks each new field, retyped field, or removed variant with `compile_error!`, so the compiler lists exactly what the author writes. `check` fails when a version has no step file.

Changes to `ProtoNode` are reported but not converted, since declarations are stored as content-addressed resources rather than in the payloads a step sees.

## API change detection

A CLI subcommand dumps `NODE_METADATA` (identifier, version, input names and types, output type) as JSON. CI dumps it for the change and for the baseline (the merge base with `master`, cached from master builds), diffs the two, and checks the registered migrations:

| Change between baseline and change | Check |
|---|---|
| An identifier disappeared | A migration from `old@version` is registered. Suggests a rename when an identical API appeared under a new identifier. |
| API changed, version unchanged | Fails, asking for a version bump. |
| Version bumped | A migration from the previous version is registered. |
| Migration registered | `target.version == source.version + 1`, and the dependency order has no cycle. |

`cargo run migration check` already covers the rows that need no baseline: every node whose `version(N)` is above 1 needs a migration leading to `identifier@N` in `migration-nodes`, and every migration must lead to a declared node or to the source of another migration. Comparing APIs against the baseline remains.

The scaffold commands, which a failing check prints, find the node in the `node-graph` sources with its identifier (the `path(...)` override or the module path, then the Pascal-case function name and `Node`) and its `version(N)`, and write into `migration-nodes`, a module per node crate and a file per step:

- `cargo run migration new` scaffolds a `#[migration("<identifier>"@V => @V+1)]` for every version between the newest one a migration leads a node to (1 if none) and its `version(N)`.
- `cargo run migration new <identifier>` does the same for one node, refusing a node still at version 1.
- `cargo run migration new <old>@<version> <identifier>` scaffolds the replacement of a removed node, checking that the new node is at `version + 1`.
- `cargo run migration rename <old>@<version> <identifier>` appends a finished `rename!` to the crate module's `renames.rs`.

A node is named by its full identifier, or by a function name or identifier suffix only one node has. The scaffolded body is a `compile_error!` naming the parameters a migration can take, so it can't ship unwritten. Migrations don't need input shapes: positions come from the anchor's instance, the source version guarantees the layout, and inserted nodes list every input explicitly.

## Testing

Every migration ships with:

- **Fixtures.** Small source graphs built in the test and assertions on the migrated result.
- **The commutation property.** For generated histories `H` over the source declaration, including concurrent branches, `replay(rewrite(H)) == M(replay(H))`. This is the definition of a correct rewrite, so a migration that passes it needs no hand-written history tests. The generator also produces the reachable source states that expose `unrepresentable` failures.

## Crate layout

```
document/migrations/
├── core/     migration-core: views, connectors, `decl!`, registry, Payload, errors, re-exporting `#[migration]` and `rename!`
├── macros/   migration-macros: `#[migration]` and `rename!`
├── formats/  migration-formats: the frozen stored types of every format version
├── snapshot/ migration-snapshot: the `cargo run migration` CLI (`snapshot`, `check`, `new`, `rename`)
├── nodes/    migration-nodes: node migrations, scaffolded by `cargo run migration new`
├── runner/   migration-runner: format chaining, ordering, history rewrite, fast path, settling, Rev mapping, translation of older peers' deltas
└── <sets>    migration crates, one per era or area, selected per binary by cargo features
```

`migration-core` depends on `serde`, `postcard`, `thiserror`, `ctor`, and `graph-storage` without default features. Node migration crates depend only on `migration-core`, and format steps also on `migration-formats`.

# Rationale and alternatives

**History rewrite vs. migration as new deltas.** New deltas keep `Rev`s stable but leave old shapes reachable through undo and replay, and corrupt concurrent edits made against the old shape (see Motivation). Fixing the latter requires translating deltas anyway.

**State functions with derived translation vs. migrations written on deltas.** Delta-level migrations must handle every delta kind that can mention a node, including indexed attribute writes, removal snapshots, and declaration resources, and a missed case silently breaks the commutation law. Deriving the rewrite as a diff of migrated states needs only `M`. The runner still gets delta-level speed for index-map migrations through the fast path, without exposing that surface to authors.

**Parameter types vs. a declared neighborhood.** A separate neighborhood attribute can disagree with what the function touches, and checking it needs runtime enforcement. Parameter types make the footprint and the permitted edits the same thing, checked by the compiler.

**Renames as migrations vs. stable identifiers.** Mandatory stable identifiers would avoid rename migrations but force every node to carry a name unrelated to its code. Renames as one-line migrations keep identifiers inferred, are detected by CI, and are covered by the fast path. The node macro's `path(...)` override remains for keeping an identifier when a node moves.

**Self-registration vs. explicit registration.** The node registry already runs `ctor` on native and exported shims on wasm, so migrations reuse proven infrastructure. Self-registration lets a binary choose migration crates by linking them, with no central list to maintain.

**Truncate always.** Simple and always possible, but it discards undo history on every node upgrade and needs every peer to agree on the truncation point.

**Frozen mirrors vs. runtime types.** Input values are stored as self-describing `graph-storage::Value`s, so a migration decodes them into its own frozen mirror. Old runtime types therefore never need to stay in the runtime crates.

# Unresolved questions

- **Timestamps inside a rewritten chain.** Either `TimeStamp` gains a sub-sequence component (`(counter, peer, sub)`), or all ops of a chain share `d`'s timestamp, which needs `compute_deltas` to write each slot at most once per diff.
- **Concurrent identical inserts.** Two branches can rewrite into an `AddNode` of the same derived ID with identical content. The CRDT must treat that as one addition.
- **External `Rev` references.** The rewrite changes `Rev`s. How long the mapping and the original history are retained (for translating older peers' deltas), and whether anything outside the document (sync server, links, comments) holds `Rev`s, are open.
- **Input attributes of inserted nodes.** `NewNode` takes bare `NodeInput`s, so inserted nodes start with empty per-input attributes. Taking `InputSlot`s would let a migration carry them over.
- **Removal safety.** `Upstream::remove_node` is only correct when nothing else reads the removed node, which the region system can't check.
- **`unrepresentable` in CI.** Whether reaching it in the commutation test fails CI outright.
- **Layout.** Whether migrations may write node positions and chain placement, or leave inserted nodes to a host-side layout pass.
- **Multiple pending migrations.** Whether consecutive pending node migrations run as separate rewrite passes or are fused into one replay.

# Future possibilities

- CLI `migrate` subcommand for batch upgrades (the runner is editor-independent).
- Online migration service running retired migration crates as wasm modules, letting active editors drop ancient migrations. Node migrations need the views as host functions.
- Declarative forms for common index maps beyond `rename!`.
- Per-library format versioning, as in the format RFC.
