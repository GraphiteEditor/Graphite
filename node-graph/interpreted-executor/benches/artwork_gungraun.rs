//! Counts the instructions for editing and viewing the demo artwork once it has been rendered, as the editor renders it after each change:
//! drawing a rectangle on top of it, which recompiles the graph and renders the viewport,
//! and zooming in, which renders the viewport anew since none of the cached tiles match the new zoom.

mod gpu_util;

use glam::{DAffine2, DVec2};
use gpu_util::{GpuDocument, VIEWPORT, callgrind};
use graph_craft::concrete;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeId, NodeInput, NodeNetwork};
use graphene_std::transform_nodes::transform;
use graphene_std::vector::generator_nodes::rectangle;
use graphene_std::vector::{GradientForm, GradientRamp};
use graphene_std::vector_nodes::fill;
use graphene_std::{Color, Context};
use gungraun::prelude::*;
use std::path::Path;

/// The zoom step the measured render takes, about the viewport center.
const ZOOM: f64 = 1.5;
/// The new rectangle's nodes, numbered clear of the document's own so every run builds the same graph.
const RECTANGLE: NodeId = NodeId(u64::MAX - 3);
const TRANSFORM: NodeId = NodeId(u64::MAX - 2);
const FILL: NodeId = NodeId(u64::MAX - 1);
const LAYER: NodeId = NodeId(u64::MAX);

pub struct Session {
	gpu: GpuDocument,
	/// Where the viewport shows the artwork.
	view: DAffine2,
	/// The document graph for the measured update.
	document: NodeNetwork,
}

/// Opens lavapipe and renders the demo artwork `name` fitted into the viewport, so the measured step meets the caches the editor would have.
fn open(name: &str) -> (Session, NodeNetwork) {
	let network = futures::executor::block_on(document_loader::open_document(Path::new(&format!("../../demo-artwork/{name}.graphite"))))
		.expect("Failed to open the demo artwork")
		.network;
	let view = fit(&network);
	let mut gpu = GpuDocument::new();
	gpu.compile(network.clone());
	gpu.render(view);
	let session = Session {
		gpu,
		view,
		document: NodeNetwork::default(),
	};
	(session, network)
}

fn setup_add_rectangle(name: &str) -> Session {
	let (mut session, network) = open(name);
	session.document = with_rectangle(network);
	session
}

/// The artwork viewed after zooming out and back, so the device has the shaders for a zoom change while no tiles at the measured zoom are cached.
fn setup_zoom(name: &str) -> Session {
	let (session, _) = open(name);
	session.gpu.render(zoomed(session.view, 1. / ZOOM));
	session.gpu.render(session.view);
	session
}

/// The view zoomed by `factor` about the viewport center.
fn zoomed(view: DAffine2, factor: f64) -> DAffine2 {
	let center = VIEWPORT.as_dvec2() / 2.;
	DAffine2::from_translation(center) * DAffine2::from_scale(DVec2::splat(factor)) * DAffine2::from_translation(-center) * view
}

/// The view that fits the document's root artboard into the viewport with a margin, as zooming to fit does.
fn fit(network: &NodeNetwork) -> DAffine2 {
	let artboard = root(network);
	let location = vec_input(artboard, 2);
	let size = vec_input(artboard, 3);
	let scale = (VIEWPORT.as_dvec2() / size).min_element() * 0.9;
	let offset = (VIEWPORT.as_dvec2() - size * scale) / 2. - location * scale;
	DAffine2::from_translation(offset) * DAffine2::from_scale(DVec2::splat(scale))
}

/// The document with a rectangle layer on top of its artboard's other layers, with the nodes the Rectangle tool creates.
fn with_rectangle(mut network: NodeNetwork) -> NodeNetwork {
	let artboard = root(&network);
	let location = vec_input(artboard, 2);
	let size = vec_input(artboard, 3);
	let NodeInput::Node { node_id: top, .. } = artboard.inputs[1] else {
		panic!("The artboard has no layers");
	};

	// The top layer serves as the template for the new one, sharing the document's layer definition
	let mut layer = network.nodes[&top].clone();
	assert!(matches!(layer.implementation, DocumentNodeImplementation::Definition(_)), "The artboard's top layer is no layer node");
	layer.inputs[0] = NodeInput::node(top, 0);
	layer.inputs[1] = NodeInput::node(FILL, 0);

	let node = |implementation, inputs| DocumentNode {
		inputs,
		call_argument: concrete!(Context),
		implementation: DocumentNodeImplementation::ProtoNode(implementation),
		..Default::default()
	};
	let value = |value| NodeInput::value(value, false);
	let rectangle = node(
		rectangle::IDENTIFIER,
		vec![
			value(TaggedValue::None),
			value(TaggedValue::Number(size.x / 3.)),
			value(TaggedValue::Number(size.y / 3.)),
			value(TaggedValue::BoxCorners(vec![0.])),
			value(TaggedValue::Bool(true)),
			value(TaggedValue::Bool(false)),
		],
	);
	let transform = node(
		transform::IDENTIFIER,
		vec![
			NodeInput::node(RECTANGLE, 0),
			value(TaggedValue::DVec2(location + size / 2.)),
			value(TaggedValue::Number(0.)),
			value(TaggedValue::DVec2(DVec2::ONE)),
			value(TaggedValue::DVec2(DVec2::ZERO)),
		],
	);
	let color = Color::from_rgbaf32_unchecked(0.9, 0.3, 0.2, 1.);
	let fill = node(
		fill::IDENTIFIER,
		vec![
			NodeInput::node(TRANSFORM, 0),
			value(TaggedValue::Color(color)),
			value(TaggedValue::Color(color)),
			value(TaggedValue::GradientRamp(GradientRamp::default())),
			value(TaggedValue::GradientForm(GradientForm::Linear)),
			value(TaggedValue::Bool(false)),
			value(TaggedValue::DAffine2(DAffine2::IDENTITY)),
		],
	);

	let NodeInput::Node { node_id: root_id, .. } = network.exports[0] else { unreachable!() };
	network.nodes.extend([(RECTANGLE, rectangle), (TRANSFORM, transform), (FILL, fill), (LAYER, layer)]);
	network.nodes.get_mut(&root_id).expect("The document has no root node").inputs[1] = NodeInput::node(LAYER, 0);
	network
}

/// The artboard the document exports, which every demo artwork has.
fn root(network: &NodeNetwork) -> &DocumentNode {
	let NodeInput::Node { node_id, .. } = network.exports[0] else {
		panic!("The document exports no node");
	};
	&network.nodes[&node_id]
}

fn vec_input(node: &DocumentNode, index: usize) -> DVec2 {
	match node.inputs[index].as_value() {
		Some(TaggedValue::DVec2(value)) => *value,
		other => panic!("The artboard's input {index} is {other:?} rather than a vector"),
	}
}

#[library_benchmark]
#[benches::artwork(args = ["isometric-fountain", "painted-dreams", "parametric-dunescape", "red-dress", "valley-of-spires"], setup = setup_add_rectangle)]
pub fn add_rectangle(mut session: Session) -> Session {
	let document = std::mem::take(&mut session.document);
	callgrind::start();
	session.gpu.compile(document);
	session.gpu.render(session.view);
	callgrind::stop();

	// Return the session so its teardown happens outside the measured section
	session
}

#[library_benchmark]
#[benches::artwork(args = ["isometric-fountain", "painted-dreams", "parametric-dunescape", "red-dress", "valley-of-spires"], setup = setup_zoom)]
pub fn zoom(session: Session) -> Session {
	let view = zoomed(session.view, ZOOM);
	callgrind::start();
	session.gpu.render(view);
	callgrind::stop();

	session
}

library_benchmark_group!(name = artwork_group; benchmarks = add_rectangle, zoom);

main!(
	config = gpu_util::config();
	library_benchmark_groups = artwork_group
);
