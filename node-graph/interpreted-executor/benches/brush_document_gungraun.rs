//! Counts the instructions a document update costs when one stroke is added to its brush layer, the way the Brush tool appends strokes:
//! wrapping and compiling the changed graph, updating the executor and rendering the viewport, as the editor's runtime does for each change.

mod brush_util;

use brush_util::{VIEWPORT, callgrind, lavapipe, strokes, wait};
use core_types::transform::{Footprint, RenderQuality};
use glam::DAffine2;
use graph_craft::application_io::{PlatformApplicationIo, PlatformEditorApi};
use graph_craft::concrete;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeInput, NodeNetwork};
use graph_craft::graphene_compiler::Compiler;
use graphene_std::application_io::RenderConfig;
use graphene_std::brush::basic_brush::{basic_brush, basic_brush_pipeline};
use graphene_std::brush::{BrushCache, Stroke, brush_strokes};
use graphene_std::graphic;
use graphene_std::runtime::{AsyncRuntime, Clock, Spawn, Task};
use graphene_std::uuid::NodeId;
use graphene_std::{Color, Context};
use gungraun::prelude::*;
use interpreted_executor::dynamic_executor::DynamicExecutor;
use interpreted_executor::util::wrap_network_in_scope;
use std::hint::black_box;
use std::sync::Arc;
use wgpu_executor::WgpuExecutor;

const STROKES_NODE: NodeId = NodeId(0);

pub struct Session {
	gpu: WgpuExecutor,
	editor_api: Arc<PlatformEditorApi>,
	runtime: AsyncRuntime,
	preprocessor: preprocessor::Preprocessor,
	executor: DynamicExecutor,
	/// The document graph with the stroke added, for the measured update.
	document: NodeNetwork,
}

impl Session {
	/// Recompiles the document graph into the executor and renders the viewport, as the runtime does after an edit.
	fn update(&mut self, document: NodeNetwork) {
		let mut network = wrap_network_in_scope(document, self.editor_api.clone(), self.runtime.handle());
		self.preprocessor.preprocess(&mut network, &|_| None).expect("Failed to preprocess the document");
		let proto_network = Compiler {}.compile_single(network).expect("Failed to compile the document").network;
		self.executor.update(proto_network).expect("Failed to update the executor");

		let render_config = RenderConfig {
			viewport: Footprint {
				transform: DAffine2::IDENTITY,
				resolution: VIEWPORT,
				quality: RenderQuality::Full,
			},
			scale: 1.,
			..Default::default()
		};
		let output = self.executor.tree().eval_tagged_value(self.executor.output(), &render_config.into_context());
		assert!(matches!(output, Ok(graphene_std::Poll::Final(_))), "Failed to render the document: {output:?}");
		wait(&self.gpu);
		let _ = black_box(output);
	}
}

/// A document of `existing` strokes rendered once, so the added stroke meets the caches the editor would have.
fn setup(existing: usize) -> Session {
	let (mut strokes, stroke) = strokes(existing);
	let gpu = lavapipe();
	let mut session = Session {
		editor_api: Arc::new(PlatformEditorApi {
			application_io: Some(Arc::new(PlatformApplicationIo::new_with_executor(gpu.clone()))),
		}),
		gpu,
		runtime: AsyncRuntime::new(Arc::new(BlockingSpawner), Arc::new(StoppedClock)),
		preprocessor: preprocessor::Preprocessor::new(),
		executor: DynamicExecutor::default(),
		document: NodeNetwork::default(),
	};

	let before = strokes.clone();
	strokes.push(stroke);

	// Rendering the finished document through a brush cache that is then discarded compiles the device's shaders and allocates its textures before anything is measured
	session.update(with_strokes(&brush_layer(BrushCache::default()), strokes.clone()));

	// The brush layer's cache lives in the graph, so it carries over between compilations like in the editor
	let template = brush_layer(BrushCache::default());
	session.update(with_strokes(&template, before));
	session.document = with_strokes(&template, strokes);
	session
}

/// A document holding just a brush layer, with the nodes and inputs the Brush tool creates and the conversion its layer applies to them.
fn brush_layer(cache: BrushCache) -> NodeNetwork {
	let strokes = DocumentNode {
		inputs: vec![
			NodeInput::value(TaggedValue::Strokes(Vec::new()), false),
			NodeInput::value(TaggedValue::Color(Color::from_rgbaf32_unchecked(0.2, 0.4, 0.8, 1.)), false),
			NodeInput::value(TaggedValue::Number(24.), false),
			NodeInput::value(TaggedValue::Number(50.), false),
			NodeInput::value(TaggedValue::Number(80.), false),
		],
		call_argument: concrete!(Context),
		implementation: DocumentNodeImplementation::ProtoNode(brush_strokes::IDENTIFIER),
		..Default::default()
	};
	let brush = DocumentNode {
		inputs: vec![
			NodeInput::node(STROKES_NODE, 0),
			NodeInput::value(TaggedValue::BrushCache(cache), false),
			NodeInput::scope(basic_brush_pipeline::IDENTIFIER),
		],
		call_argument: concrete!(Context),
		implementation: DocumentNodeImplementation::ProtoNode(basic_brush::IDENTIFIER),
		..Default::default()
	};
	// The layer node takes its content through `Into Group` before merging it into the stack
	let group = DocumentNode {
		inputs: vec![NodeInput::node(NodeId(1), 0)],
		call_argument: concrete!(Context),
		implementation: DocumentNodeImplementation::ProtoNode(graphic::into_group::IDENTIFIER),
		..Default::default()
	};
	NodeNetwork {
		exports: vec![NodeInput::node(NodeId(2), 0)],
		nodes: [(STROKES_NODE, strokes), (NodeId(1), brush), (NodeId(2), group)].into_iter().collect(),
		..Default::default()
	}
}

fn with_strokes(template: &NodeNetwork, strokes: Vec<Stroke>) -> NodeNetwork {
	let mut document = template.clone();
	let node = document.nodes.get_mut(&STROKES_NODE).expect("The brush layer has no Brush Strokes node");
	node.inputs[0] = NodeInput::value(TaggedValue::Strokes(strokes), false);
	document
}

/// Runs spawned work to completion on the spot, so everything the graph starts happens in the measured section.
struct BlockingSpawner;

impl Spawn for BlockingSpawner {
	fn spawn(&self, task: Task) {
		futures::executor::block_on(task);
	}
}

/// Keeps the same time throughout, so animation never depends on how slowly Valgrind runs.
struct StoppedClock;

impl Clock for StoppedClock {
	fn now(&self) -> std::time::Duration {
		std::time::Duration::ZERO
	}
}

#[library_benchmark]
#[benches::existing_strokes(args = [0, 16, 256], setup = setup)]
pub fn add_stroke(mut session: Session) -> Session {
	let document = std::mem::take(&mut session.document);
	callgrind::start();
	session.update(document);
	callgrind::stop();

	// Return the session so its teardown happens outside the measured section
	session
}

library_benchmark_group!(name = brush_document_group; benchmarks = add_stroke);

main!(
	config = brush_util::config();
	library_benchmark_groups = brush_document_group
);
