//! Temporary, uncommitted: times edit loops (nudge a Transform node, recompile, update the executor, evaluate) on demo artwork.
//!
//! Usage: `[VELLO=1] cargo run --release -p interpreted-executor --example profile_edits -- <iterations> [document names...]`
//! With `VELLO=1` it renders through Vello on a GPU context instead of to SVG.

use core_types::uuid::NodeId;
use glam::{DAffine2, DVec2, UVec2};
use graph_craft::application_io::{PlatformApplicationIo, PlatformEditorApi};
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNodeImplementation, NodeInput, NodeNetwork};
use graph_craft::proto::ProtoNetwork;
use graph_craft::util::load_network;
use graphene_std::application_io::{ExportFormat, RenderConfig};
use graphene_std::runtime::{AsyncRuntime, Spawn, Task};
use graphene_std::transform::Footprint;
use interpreted_executor::dynamic_executor::DynamicExecutor;
use interpreted_executor::util::wrap_network_in_scope;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

struct DummySpawner;
impl Spawn for DummySpawner {
	fn spawn(&self, _task: Task) {}
}

fn compile(network: NodeNetwork, editor_api: &Arc<PlatformEditorApi>) -> ProtoNetwork {
	let mut network = wrap_network_in_scope(network, editor_api.clone(), AsyncRuntime::new(std::sync::Arc::new(DummySpawner)).handle());
	preprocessor::Preprocessor::new().preprocess(&mut network, &|_| None).unwrap();
	let mut proto = graph_craft::util::compile(network);
	match std::env::var("MEMO_EXTEND").as_deref() {
		Ok("1") => memoize_extend_outputs(&mut proto),
		Ok("base") => memoize_extend_bases(&mut proto),
		_ => {}
	}
	proto
}

/// Puts a Memoize node on the base (lower stack) input of every Extend node, leaving the Extend's other consumers of that input alone.
fn memoize_extend_bases(proto: &mut ProtoNetwork) {
	let Some(template) = proto.nodes.iter().find(|(_, node)| node.identifier.as_str().ends_with("MemoizeNode")).map(|(_, node)| node.clone()) else {
		return;
	};
	let memo_of = |id: NodeId| NodeId(id.0.rotate_left(23) ^ 0x6a09_e667_f3bc_c908);

	// Rewire each Extend's base to the memo of that base
	let mut bases = std::collections::HashSet::new();
	for (_, node) in &mut proto.nodes {
		if node.identifier.as_str().ends_with("ExtendNode")
			&& let graph_craft::proto::ConstructionArgs::Nodes(inputs) = &mut node.construction_args
			&& let Some(base) = inputs.first_mut()
		{
			bases.insert(*base);
			*base = memo_of(*base);
		}
	}

	// Insert each memo right after its base to keep the list topologically sorted
	let mut nodes = Vec::with_capacity(proto.nodes.len() + bases.len());
	for (id, node) in std::mem::take(&mut proto.nodes) {
		let original_location = node.original_location.clone();
		nodes.push((id, node));
		if bases.contains(&id) {
			let mut memo = template.clone();
			memo.construction_args = graph_craft::proto::ConstructionArgs::Nodes(vec![id]);
			memo.original_location = original_location;
			nodes.push((memo_of(id), memo));
		}
	}
	proto.nodes = nodes;
	eprintln!("memoized {} extend bases", bases.len());
}

/// Puts a Memoize node after every Extend node, with an ID derived from the Extend's so it stays stable across recompiles.
fn memoize_extend_outputs(proto: &mut ProtoNetwork) {
	let Some(template) = proto.nodes.iter().find(|(_, node)| node.identifier.as_str().ends_with("MemoizeNode")).map(|(_, node)| node.clone()) else {
		return;
	};

	let extends: Vec<NodeId> = proto.nodes.iter().filter(|(_, node)| node.identifier.as_str().ends_with("ExtendNode")).map(|(id, _)| *id).collect();
	let memo_of = |id: NodeId| NodeId(id.0.rotate_left(17) ^ 0x9e37_79b9_7f4a_7c15);

	// Point every consumer of an Extend at its memo
	let extend_set: std::collections::HashSet<NodeId> = extends.iter().copied().collect();
	for (_, node) in &mut proto.nodes {
		node.map_ids(|id| if extend_set.contains(&id) { memo_of(id) } else { id });
	}
	if extend_set.contains(&proto.output) {
		proto.output = memo_of(proto.output);
	}

	// Insert each memo right after its Extend to keep the list topologically sorted
	let mut nodes = Vec::with_capacity(proto.nodes.len() + extends.len());
	for (id, node) in std::mem::take(&mut proto.nodes) {
		let is_extend = extend_set.contains(&id);
		let original_location = node.original_location.clone();
		nodes.push((id, node));
		if is_extend {
			let mut memo = template.clone();
			memo.construction_args = graph_craft::proto::ConstructionArgs::Nodes(vec![id]);
			memo.original_location = original_location;
			nodes.push((memo_of(id), memo));
		}
	}
	proto.nodes = nodes;
	eprintln!("memoized {} extend outputs", extends.len());
}

fn for_each_translation(network: &mut NodeNetwork, visit: &mut impl FnMut(&mut DVec2)) {
	let mut node_ids: Vec<NodeId> = network.nodes.keys().copied().collect();
	node_ids.sort();

	for node_id in node_ids {
		let Some(node) = network.nodes.get_mut(&node_id) else { continue };
		match &mut node.implementation {
			DocumentNodeImplementation::Network(nested) => for_each_translation(nested, visit),
			DocumentNodeImplementation::ProtoNode(identifier) if identifier.as_str().ends_with("TransformNode") => {
				if let Some(NodeInput::Value { tagged_value, .. }) = node.inputs.get_mut(1)
					&& let TaggedValue::DVec2(translation) = &mut *tagged_value.inner_mut()
				{
					visit(translation);
				}
			}
			_ => {}
		}
	}
}

fn node_input_id(input: Option<&NodeInput>) -> Option<NodeId> {
	match input {
		Some(NodeInput::Node { node_id, .. }) => Some(*node_id),
		_ => None,
	}
}

fn is_fill(node: &graph_craft::document::DocumentNode) -> bool {
	matches!(&node.implementation, DocumentNodeImplementation::ProtoNode(identifier) if identifier.as_str().ends_with("FillNode"))
}

/// Copies the node and everything it reads from in the same network under fresh IDs, returning the copy's ID.
fn copy_subtree(network: &mut NodeNetwork, root: NodeId, next_id: &mut u64) -> NodeId {
	let mut copied = std::collections::HashMap::new();
	let mut pending = vec![root];
	while let Some(node_id) = pending.pop() {
		if copied.contains_key(&node_id) {
			continue;
		}
		*next_id += 1;
		copied.insert(node_id, NodeId(*next_id));
		pending.extend(network.nodes[&node_id].inputs.iter().filter_map(|input| node_input_id(Some(input))));
	}

	for (original, copy) in &copied {
		let mut node = network.nodes[original].clone();
		for input in &mut node.inputs {
			if let NodeInput::Node { node_id, .. } = input {
				*node_id = copied[node_id];
			}
		}
		network.nodes.insert(*copy, node);
	}
	copied[&root]
}

/// Draws a shape layer on top of (or with `at_bottom`, beneath) the first artboard's content, as dragging a newly drawn shape needs, and returns its Transform node's ID.
/// The layer is a copy of the artboard's top layer, its shape a copy of the bottom-most layer's Fill chain, placed by a copy of `transform_template`.
fn insert_shape(network: &mut NodeNetwork, transform_template: &graph_craft::document::DocumentNode, at_bottom: bool) -> Option<NodeId> {
	let artboard = node_input_id(network.exports.first())?;
	let old_top = node_input_id(network.nodes[&artboard].inputs.get(1))?;

	// The first layer down the stack is the template for the new one, and the last one with a Fill content gives its shape
	let mut layer_template = None;
	let mut bottom_layer = None;
	let mut shape = None;
	let mut current = Some(old_top);
	while let Some(node_id) = current {
		let node = &network.nodes[&node_id];
		if matches!(node.implementation, DocumentNodeImplementation::Network(_)) && node.inputs.len() == 2 {
			layer_template.get_or_insert(node_id);
			bottom_layer = Some(node_id);
			if let Some(content) = node_input_id(node.inputs.get(1))
				&& is_fill(&network.nodes[&content])
			{
				shape = Some(content);
			}
		}
		current = node_input_id(node.inputs.first());
	}
	let (layer_template, shape) = (layer_template?, shape?);

	let mut next_id = 0x5ade_0000_0000;
	let shape_copy = copy_subtree(network, shape, &mut next_id);

	let transform_id = NodeId(next_id + 1);
	let mut transform = transform_template.clone();
	transform.inputs[0] = NodeInput::node(shape_copy, 0);
	network.nodes.insert(transform_id, transform);

	let layer_id = NodeId(next_id + 2);
	let mut layer = network.nodes[&layer_template].clone();
	layer.inputs[1] = NodeInput::node(transform_id, 0);

	// Beneath the stack, the new layer takes over the bottom layer's stack input, so every layer above it is downstream of the drag
	if at_bottom {
		let bottom_layer = bottom_layer?;
		layer.inputs[0] = network.nodes[&bottom_layer].inputs[0].clone();
		network.nodes.insert(layer_id, layer);
		network.nodes.get_mut(&bottom_layer)?.inputs[0] = NodeInput::node(layer_id, 0);
	} else {
		layer.inputs[0] = NodeInput::node(old_top, 0);
		network.nodes.insert(layer_id, layer);
		network.nodes.get_mut(&artboard)?.inputs[1] = NodeInput::node(layer_id, 0);
	}
	Some(transform_id)
}

/// Moves the Transform node with the given ID in the root network by 10 units.
fn nudge_transform(network: &mut NodeNetwork, transform_id: NodeId) {
	if let Some(NodeInput::Value { tagged_value, .. }) = network.nodes.get_mut(&transform_id).and_then(|node| node.inputs.get_mut(1))
		&& let TaggedValue::DVec2(translation) = &mut *tagged_value.inner_mut()
	{
		*translation += DVec2::new(10., 0.);
	}
}

/// The only Transform node with a translation value among the demo artwork, used as the template for inserted shapes.
fn transform_template() -> graph_craft::document::DocumentNode {
	let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../demo-artwork/changing-seasons.graphite");
	let network = load_network(&std::fs::read_to_string(&path).expect("Failed to read the document"));
	let artboard = node_input_id(network.exports.first()).expect("The document has an artboard");
	let transform = node_input_id(network.nodes[&artboard].inputs.get(1)).expect("The artboard has content");
	network.nodes[&transform].clone()
}

/// Prints the chain of nodes from the network's export down its primary inputs, with each node's inputs.
fn dump_stack(network: &NodeNetwork, depth: usize) {
	let describe = |node: &graph_craft::document::DocumentNode| match &node.implementation {
		DocumentNodeImplementation::ProtoNode(identifier) => identifier.as_str().rsplit("::").next().unwrap_or_default().to_string(),
		DocumentNodeImplementation::Network(nested) => format!("Network({} nodes)", nested.nodes.len()),
		_ => "other".into(),
	};
	let indent = "  ".repeat(depth);

	let mut current = network
		.exports
		.first()
		.and_then(|export| if let NodeInput::Node { node_id, .. } = export { Some(*node_id) } else { None });
	let mut steps = 0;
	while let Some(node_id) = current
		&& steps < 40
	{
		let Some(node) = network.nodes.get(&node_id) else { break };
		let inputs: Vec<String> = node
			.inputs
			.iter()
			.map(|input| match input {
				NodeInput::Node { node_id, .. } => network.nodes.get(node_id).map(describe).unwrap_or_default(),
				NodeInput::Value { tagged_value, .. } => format!("value {}", tagged_value.ty()),
				NodeInput::Import { import_index, .. } => format!("import {import_index}"),
				_ => "other".into(),
			})
			.collect();
		println!("{indent}{node_id} {} [{}]", describe(node), inputs.join(", "));

		// The artboard's content is its second input, so descend into it rather than to the artboards below
		let next_index = if steps == 0 { 1 } else { 0 };
		current = node
			.inputs
			.get(next_index)
			.and_then(|input| if let NodeInput::Node { node_id, .. } = input { Some(*node_id) } else { None });
		steps += 1;
	}
}

/// Places `factor` times as many points along every Sample Polyline node's path, by dividing its separation and multiplying its quantity.
fn densify_sampling(network: &mut NodeNetwork, factor: f64) {
	for node in network.nodes.values_mut() {
		match &mut node.implementation {
			DocumentNodeImplementation::Network(nested) => densify_sampling(nested, factor),
			DocumentNodeImplementation::ProtoNode(identifier) if identifier.as_str().ends_with("SamplePolylineNode") => {
				if let Some(NodeInput::Value { tagged_value, .. }) = node.inputs.get_mut(2)
					&& let TaggedValue::Number(separation) = &mut *tagged_value.inner_mut()
				{
					*separation /= factor;
				}
				if let Some(NodeInput::Value { tagged_value, .. }) = node.inputs.get_mut(3)
					&& let TaggedValue::Integer(quantity) = &mut *tagged_value.inner_mut()
				{
					*quantity = (*quantity as f64 * factor) as i64;
				}
			}
			_ => {}
		}
	}
}

/// Nudges the middle Transform node by traversal order, as the trace driver's `edit_middle` frame does.
fn nudge_middle_transform(network: &mut NodeNetwork) -> bool {
	let mut count = 0;
	for_each_translation(network, &mut |_| count += 1);
	if count == 0 {
		return false;
	}

	let target = std::env::var("NUDGE_INDEX").ok().and_then(|value| value.parse().ok()).unwrap_or((count - 1) / 2);
	let mut index = 0;
	for_each_translation(network, &mut |translation| {
		if index == target {
			*translation += DVec2::new(10., 0.);
		}
		index += 1;
	});
	true
}

fn main() {
	let mut arguments = std::env::args().skip(1);
	let iterations: usize = arguments.next().and_then(|value| value.parse().ok()).unwrap_or(60);
	let names: Vec<String> = arguments.collect();
	let vello = std::env::var("VELLO").is_ok_and(|value| value == "1");

	// Vello renders on a GPU context, which the render node takes from the editor API's application IO
	let editor_api = Arc::new(match vello {
		true => PlatformEditorApi {
			application_io: Some(Arc::new(futures::executor::block_on(PlatformApplicationIo::new()))),
		},
		false => PlatformEditorApi::default(),
	});

	let context = RenderConfig {
		viewport: Footprint {
			transform: DAffine2::IDENTITY,
			resolution: UVec2::new(1920, 1080),
			..Default::default()
		},
		scale: 1.,
		export_format: if vello { ExportFormat::Raster } else { ExportFormat::Svg },
		..Default::default()
	}
	.into_context();

	for name in names {
		let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../demo-artwork").join(format!("{name}.graphite"));
		let mut network = load_network(&std::fs::read_to_string(&path).expect("Failed to read the document"));

		if std::env::var("DUMP").is_ok_and(|value| value == "1") {
			dump_stack(&network, 0);
			continue;
		}

		if let Ok(position) = std::env::var("NUDGE_CHECK")
			&& (position == "top" || position == "bottom")
		{
			let render = |network: &NodeNetwork| {
				let executor = DynamicExecutor::new(compile(network.clone(), &editor_api)).expect("The document compiles");
				format!("{:?}", executor.tree().eval_tagged_value(executor.output(), &context)).len()
			};
			let without_shape = render(&network);
			let transform_id = insert_shape(&mut network, &transform_template(), position == "bottom").expect("The document has a shape layer to copy");
			let with_shape = render(&network);

			let baseline: std::collections::HashSet<NodeId> = compile(network.clone(), &editor_api).nodes.iter().map(|(id, _)| *id).collect();
			nudge_transform(&mut network, transform_id);
			let changed = compile(network.clone(), &editor_api).nodes.iter().filter(|(id, _)| !baseline.contains(id)).count();
			println!("{name}: output length {without_shape} without the shape, {with_shape} with it; a nudge makes {changed} new proto nodes");
			continue;
		}

		if std::env::var("NUDGE_CHECK").is_ok_and(|value| value == "1") {
			let baseline: std::collections::HashSet<NodeId> = compile(network.clone(), &editor_api).nodes.iter().map(|(id, _)| *id).collect();
			let mut count = 0;
			for_each_translation(&mut network, &mut |_| count += 1);
			for target in 0..count {
				let mut nudged = network.clone();
				let mut index = 0;
				for_each_translation(&mut nudged, &mut |translation| {
					if index == target {
						*translation += DVec2::new(10., 0.);
					}
					index += 1;
				});
				let changed = compile(nudged, &editor_api).nodes.iter().filter(|(id, _)| !baseline.contains(id)).count();
				println!("{name} transform {target}: {changed} new proto nodes");
			}
			continue;
		}
		if let Some(factor) = std::env::var("DENSIFY").ok().and_then(|value| value.parse::<f64>().ok()) {
			densify_sampling(&mut network, factor);
		}
		// With `NUDGE=top`, a shape drawn on top of the artwork is dragged, as when moving a new shape over it in the editor.
		// With `NUDGE=bottom`, the shape is beneath every layer instead, so everything above it is recomputed each edit.
		let top_shape_transform = match std::env::var("NUDGE").as_deref() {
			Ok("top") => Some(insert_shape(&mut network, &transform_template(), false).expect("The document has a shape layer to copy")),
			Ok("bottom") => Some(insert_shape(&mut network, &transform_template(), true).expect("The document has a shape layer to copy")),
			_ => None,
		};

		let mut executor = DynamicExecutor::new(compile(network.clone(), &editor_api)).expect("The document compiles");
		let _ = executor.tree().eval_tagged_value(executor.output(), &context);

		let (mut update_time, mut eval_time) = (Duration::ZERO, Duration::ZERO);
		for _ in 0..iterations {
			match top_shape_transform {
				Some(transform_id) => nudge_transform(&mut network, transform_id),
				None if !nudge_middle_transform(&mut network) => break,
				None => {}
			}

			let start = Instant::now();
			if executor.update(compile(network.clone(), &editor_api)).is_err() {
				break;
			}
			update_time += start.elapsed();

			let start = Instant::now();
			let _ = executor.tree().eval_tagged_value(executor.output(), &context);
			eval_time += start.elapsed();
		}

		// The process's resident memory now and at its peak, which run one document per process to attribute
		let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
		let status_megabytes = |field: &str| {
			let kilobytes: f64 = status
				.lines()
				.find(|line| line.starts_with(field))
				.and_then(|line| line.split_whitespace().nth(1)?.parse().ok())
				.unwrap_or_default();
			kilobytes / 1024.
		};

		let per_edit = |duration: Duration| duration.as_secs_f64() * 1e3 / iterations as f64;
		println!(
			"{name:24} compile+update {:7.2} ms/edit   evaluate {:7.2} ms/edit   resident {:6.1} MB (peak {:6.1} MB)",
			per_edit(update_time),
			per_edit(eval_time),
			status_megabytes("VmRSS:"),
			status_megabytes("VmHWM:")
		);
	}
}
