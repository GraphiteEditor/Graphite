+++
title = "Document migrations"

[extra]
order = 2 # Page number after chapter intro
+++

A document saved today has to open after you change the node it uses. A migration is the code that upgrades it: when a `.gdd` opens, the format steps run on its stored bytes, and then node migrations rewrite its history, undo included, so replaying that history gives the upgraded document at every step. The design and its reasoning are in the [migrations RFC](https://github.com/GraphiteEditor/Graphite/blob/master/node-graph/rfcs/document-format-migrations.md), which also describes parts that don't exist yet. This page covers what you can use now.

Most changes need a node migration, written as a function in `document/migrations/nodes`. If you changed a type under `document/graph-storage/src/schema`, you need a format step instead, and the tooling writes most of it for you (see [format steps](#format-steps)).

## Node migrations

Bump a node's version when a document saved before your change would load wrongly after it. Adding, removing, reordering, or retyping an input counts. So does changing what an output means, moving the node to a new identifier, or replacing it with another node. A behavior change with the same inputs counts too, like reading an angle in degrees instead of radians. An internal refactor doesn't.

CI catches the changes it can see: it compares every node's parameters and output against the base commit, and fails if they differ without a version bump or if a node disappeared without a migration. It compares names as well as types, so renaming a parameter also asks for a bump. It can't see behavior changes, so those bumps are on you. Run the same check locally with `cargo run migration check --baseline master`.

### Writing one

Start by changing the node and bumping its version. Versions start at 1 when unset, and each migration moves exactly one version:

```rust
#[node_macro::node(category("Vector: Geometry"), icon("NodeStar"), version(2))]
fn star(/* ... */) -> Item<Vector> {
	// ...
}
```

Then scaffold the migration:

```sh
cargo run migration new
```

This finds every node whose version is ahead of its newest migration and writes one file per missing step, like `document/migrations/nodes/src/vector_nodes/star_v1.rs`. To scaffold a single node, pass its identifier, or its function name if only one node has that name.

The generated file won't compile until you fill it in. `cargo check` stops at the `compile_error!` in the function body, and `cargo test` stops at the ones in its test. Replace the placeholder doc comment with what actually changed.

A migration runs once for each node of the old version, called the anchor. The runner switches the anchor to the new version itself, so the function only describes what else has to change. This one doubles a stored radius:

```rust
use migration_core::{Inputs, MigrationError, Node, NodeInput, encode, literal_of, migration};
use serde::{Deserialize, Serialize};

/// Frozen mirror of the `TaggedValue` variants this migration reads and writes.
#[derive(Serialize, Deserialize)]
enum Tagged {
	Number(f64),
}

const RADIUS_1: u32 = 2;

/// Radius 1 now means half the distance, so stored radii double to keep their shape.
#[migration("vector_nodes::generator_nodes::StarNode"@1 => @2)]
pub fn star_v1(node: &Node, inputs: &mut Inputs) -> Result<(), MigrationError> {
	let radius = &node.inputs()[RADIUS_1 as usize].input;

	// A wired input has no stored value to convert. "Adding nodes" below handles that case
	if let NodeInput::Value { exposed, .. } = radius {
		let Some(Tagged::Number(radius)) = literal_of(radius) else {
			return Err(MigrationError::invariant("radius 1 is not a number"));
		};
		inputs.set(
			RADIUS_1,
			NodeInput::Value {
				value: encode(&Tagged::Number(radius * 2.)),
				exposed: *exposed,
			},
		);
	}

	Ok(())
}
```

Input slots count every parameter after the context, in order, including the primary input. In `fn star(_: impl Ctx, _primary: (), sides, radius_1, radius_2)`, `radius_1` is slot 2.

### Parameters

You ask for what you need through the function's parameters, in any order. The function can't be async or generic, and returns either nothing or `Result<(), MigrationError>`.

| Parameter | Gives you | Operations |
|---|---|---|
| `&Node` | The anchor before migration | `id()`, `inputs()`, and the stored node's other accessors |
| `&Graph` | The anchor's network before migration | `node(id)`, `declaration(id)`, `consumers(output)` |
| `&mut Inputs` | The anchor's input slots | `set`, `insert`, `push`, `remove`, `truncate`, `len` |
| `&mut Upstream` | Nodes feeding the anchor | `add_node`, `add_node_keyed`, `remove_node` |
| `&mut Downstream` | Whatever reads the anchor's outputs | `consumers`, `add_node`, `add_node_keyed`, `connect` |

Reads always see the old document. `node.inputs()` returns the old layout however many edits you've made, and nodes you add never show up in `consumers`. `Inputs` edits, on the other hand, apply in order to the layout being built: after `inputs.insert(1, ...)`, the old slot 1 is slot 2 for later `Inputs` calls, while `node.inputs()[1]` is still the old slot 1.

### Values

A migration has to keep working after the code around it changes, so it can't use live types like `TaggedValue`. Declare an enum with only the variants you read or write, named the same as in `TaggedValue`:

```rust
use serde::{Deserialize, Serialize};

/// Frozen mirror of the `TaggedValue` variants this migration uses.
#[derive(Serialize, Deserialize)]
enum Tagged {
	Number(f64),
	Integer(i64),
	Bool(bool),
}
```

`literal(value)` makes an unexposed literal input, and `encode(&value)` makes the stored value when you're building a `NodeInput::Value` that keeps its `exposed` flag. `literal_of::<Tagged>(&input)` reads one back, and gives `None` for a wire or for a variant your enum doesn't list.

### Adding nodes

A wired input is only known at runtime, so converting it takes a new node. Every input of the new node has to be given, because nothing fills in defaults:

```rust
use migration_core::{Inputs, NewNode, Node, NodeInput, Upstream, decl, literal, migration};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
enum Tagged {
	Number(f64),
}

const RADIUS_1: u32 = 2;

/// Radius 1 now means half the distance, so it reads through a doubling.
#[migration("vector_nodes::generator_nodes::StarNode"@1 => @2)]
pub fn star_v1(node: &Node, inputs: &mut Inputs, upstream: &mut Upstream) {
	let radius = node.inputs()[RADIUS_1 as usize].input.clone();

	let doubled = upstream.add_node(NewNode {
		declaration: decl!("math_nodes::MultiplyNode"@1),
		inputs: vec![radius, literal(Tagged::Number(2.))],
	});

	inputs.set(RADIUS_1, NodeInput::Node { id: doubled, index: 0 });
}
```

`decl!` names the exact version you built `inputs` for. The runner orders migrations so the node you add has that shape, and any later migration of `MultiplyNode` upgrades it along with all the others, so you never go back and edit an old migration. Adding two nodes of one kind for one anchor needs a different key on each: `add_node_keyed(node, "x")`.

If the output is what changed, put the new node after the anchor and move its consumers over:

```rust
use migration_core::{Downstream, NewNode, Node, NodeInput, OutputConnector, decl, literal, migration};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

#[derive(Serialize, Deserialize)]
enum Tagged {
	Number(f64),
}

/// The output is now in degrees, so existing consumers keep receiving radians through a conversion.
#[migration("math_nodes::AngleBetweenNode"@1 => @2)]
pub fn angle_between_v1(node: &Node, downstream: &mut Downstream) {
	let consumers = downstream.consumers(0);
	if consumers.is_empty() {
		return;
	}

	let to_radians = downstream.add_node(NewNode {
		declaration: decl!("math_nodes::MultiplyNode"@1),
		inputs: vec![NodeInput::Node { id: node.id(), index: 0 }, literal(Tagged::Number(PI / 180.))],
	});
	for consumer in consumers {
		downstream.connect(OutputConnector { node: to_radians, output: 0 }, consumer);
	}
}
```

### Renames and replacements

A node that only moved keeps its inputs, so it needs no function. Set its version one above the old one and register the rename, which adds a line to that crate's `renames.rs`:

```sh
cargo run migration rename graphene_core::ops::AddNode@1 math_nodes::AddNode
```

```rust
use migration_core::rename;

rename!("graphene_core::ops::AddNode"@1 => "math_nodes::AddNode"@2);
```

When the replacement also changes the inputs, scaffold a function with `cargo run migration new <old identifier>@<version> <identifier>`. The new node has to be at `version + 1`.

### What the function may depend on

A migration runs many times on one document, once per relevant step of its history, and every collaborator has to get the same result. So it can only depend on its parameters: no randomness, no clocks, no global state, no calls into live node code.

It also has to handle every state the old version allowed, including wired inputs and exposed literals. Return `MigrationError::invariant` if the old state breaks its own node's rules, and `MigrationError::unrepresentable` if the new version can't express something the old one could, which usually means the new version should change. Both are bugs, and right now either one stops the document from opening.

### Testing

The scaffolded file has a test that runs your migration over generated histories. `check_commutation` checks that rewriting a history and replaying it gives the same document as migrating the end result, and `check_undo` does the same after undoing. All you supply is what the old version allows:

```rust
#[cfg(test)]
mod tests {
	use super::*;
	use migration_core::literal;
	use migration_runner::testing::{SourceContract, check_commutation, check_undo};

	fn contract() -> SourceContract {
		SourceContract::new(4, |random, _slot| literal(Tagged::Number(random.below(10) as f64)))
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&StarV1, &contract());
		check_undo(&StarV1, &contract());
	}
}
```

`#[migration]` generates the `StarV1` struct from the function's name. The first argument of `SourceContract::new` is how many inputs the old version stores, and the closure returns a literal it would allow in that slot. If a slot can't be wired to another node of the same kind, set `reads_own_output` on the contract.

For specific cases, `migration_runner::testing::Fixture` builds a small graph you can migrate and inspect, and `document/migrations/runner/tests/node_migrations` has examples. Run them with `cargo test -p migration-nodes`.

To try it on a real document, export one with `-v`, which logs `Migrated nodes from <identifier>@<version>` as it opens:

```sh
cargo run --package graphene-cli -- -v export document.gdd -o result.png
```

## Format steps

You need one when `cargo run migration check` reports that the stored types changed. After changing the type, freeze it as the next format version:

```sh
cargo run migration snapshot
```

This writes `document/migrations/formats/src/v<N>.rs` and the step `steps/v<N-1>_to_v<N>.rs`, and sets `SUPPORTED_FORMAT_VERSION`. Types that only contain a changed type convert on their own. Each new field, retyped field, or removed variant is a `compile_error!` naming what to write:

```rust
impl Upgrade for v1::ExportSlot {
	type Next = v2::ExportSlot;
	fn upgrade(self) -> Result<Self::Next, MigrationError> {
		let v1::ExportSlot { target, timestamp } = self;
		Ok(v2::ExportSlot {
			label: String::new(),
			target,
			timestamp,
		})
	}
}
```

Finish with `cargo run migration check` and `cargo test -p migration-formats`, whose tests encode live documents and decode them through the newest frozen version. Don't edit a frozen version once it's released, because old documents decode through it. Comments and formatting in the `schema` module don't count as changes.

`ProtoNode` is the exception. Declarations are stored as resources rather than in the payloads a step converts, so `snapshot` warns when `ProtoNode` changes, and any new field has to be optional.

## Regression corpus

The [corpus](https://github.com/tagtraumen/graphite-corpus) is a separate repository of real documents and the images they should render, and CI checks every pull request against it. A correct migration keeps a document looking the same, so `check` should still pass with your change:

```sh
cargo run regression --corpus <directory> check
```

On a mismatch, the render and a diff image go to `target/regression`. Setting `GRAPHITE_REGRESSION_CORPUS` saves passing `--corpus` every time. To add a document, put the `.gdd` in the corpus's `ingest` folder, run `ingest`, and commit the result. Its images are stored with Git LFS, so you need `git-lfs` installed. After an intended visual change, `bless <name>` accepts the new render.

The demo artwork works differently. Its `.gdd` files are kept at the current version, so a migration that affects them means running `cargo run regression upgrade` and committing the result, which CI checks. The corpus documents stay as they were added, since they exist to prove that old files still upgrade.

## Current limits

A document whose history contains merges fails to migrate. Nodes generated by `#[derive(Destructure)]` can't be scaffolded, because they have no function of their own to carry a version. And when a migration fails, the document doesn't open: the RFC's fallback of truncating history isn't built yet.
