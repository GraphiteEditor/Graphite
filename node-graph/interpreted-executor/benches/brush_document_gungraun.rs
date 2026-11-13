//! Counts the instructions a document update costs when one stroke is added to its brush layer, the way the Brush tool appends strokes:
//! wrapping and compiling the changed graph, updating the executor and rendering the viewport, as the editor's runtime does for each change.

mod brush_util;

use brush_util::{Group, VIEWPORT, callgrind, document, lavapipe, wait};
use core_types::transform::{Footprint, RenderQuality};
use glam::DAffine2;
use graph_craft::application_io::{PlatformApplicationIo, PlatformEditorApi};
use graph_craft::concrete;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeInput, NodeNetwork};
use graph_craft::graphene_compiler::Compiler;
use graphene_std::Context;
use graphene_std::application_io::RenderConfig;
use graphene_std::brush::basic_brush::{basic_brush, basic_brush_pipeline};
use graphene_std::brush::{BrushCache, brush_strokes};
use graphene_std::graphic;
use graphene_std::runtime::{AsyncRuntime, Clock, Spawn, Task};
use graphene_std::uuid::NodeId;
use gungraun::prelude::*;
use interpreted_executor::dynamic_executor::DynamicExecutor;
use interpreted_executor::util::wrap_network_in_scope;
use std::hint::black_box;
use std::sync::Arc;
use wgpu_executor::WgpuExecutor;

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
	let (before, after) = document(existing);
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

	// Rendering the finished document through a brush cache that is then discarded compiles the device's shaders and allocates its textures before anything is measured
	session.update(brush_layer(&BrushCache::default(), &after));

	// The brush layer's cache lives in the graph, so it carries over between compilations like in the editor
	let cache = BrushCache::default();
	session.update(brush_layer(&cache, &before));
	session.document = brush_layer(&cache, &after);
	session
}

/// The `Basic Brush` node and the layer conversion after it, whose IDs stay put as groups are added.
const BRUSH_NODE: NodeId = NodeId(0);
const LAYER_NODE: NodeId = NodeId(1);

/// A document holding just a brush layer with its `Brush Strokes` groups,
/// stacked with the nodes the `Merge` layer uses and ending in the conversion the brush layer applies to its content.
/// Each group keeps its node IDs as strokes are added, as in the editor.
fn brush_layer(cache: &BrushCache, groups: &[Group]) -> NodeNetwork {
	let node = |implementation, inputs| DocumentNode {
		inputs,
		call_argument: concrete!(Context),
		implementation: DocumentNodeImplementation::ProtoNode(implementation),
		..Default::default()
	};
	let id = |index: usize, part: u64| NodeId(10 + 3 * index as u64 + part);

	let mut nodes = Vec::new();
	let mut stack = None;
	for (index, group) in groups.iter().enumerate() {
		let strokes = node(
			brush_strokes::IDENTIFIER,
			vec![
				NodeInput::value(TaggedValue::Strokes(group.strokes.clone()), false),
				NodeInput::value(TaggedValue::Color(group.style.color), false),
				NodeInput::value(TaggedValue::Number(group.style.diameter), false),
				NodeInput::value(TaggedValue::Number(group.style.hardness), false),
				NodeInput::value(TaggedValue::Number(group.style.flow), false),
			],
		);
		nodes.push((id(index, 0), strokes));
		nodes.push((id(index, 1), node(graphic::into_group::IDENTIFIER, vec![NodeInput::node(id(index, 0), 0)])));
		stack = Some(match stack {
			None => id(index, 1),
			Some(below) => {
				nodes.push((id(index, 2), node(graphic::extend::IDENTIFIER, vec![NodeInput::node(below, 0), NodeInput::node(id(index, 1), 0)])));
				id(index, 2)
			}
		});
	}

	let stack = stack.expect("A brush layer has at least one group");
	let brush = node(
		basic_brush::IDENTIFIER,
		vec![
			NodeInput::node(stack, 0),
			NodeInput::value(TaggedValue::BrushCache(cache.clone()), false),
			NodeInput::scope(basic_brush_pipeline::IDENTIFIER),
		],
	);
	nodes.push((BRUSH_NODE, brush));
	// The layer node takes its content through `Into Group` before merging it into the stack
	nodes.push((LAYER_NODE, node(graphic::into_group::IDENTIFIER, vec![NodeInput::node(BRUSH_NODE, 0)])));

	NodeNetwork {
		exports: vec![NodeInput::node(LAYER_NODE, 0)],
		nodes: nodes.into_iter().collect(),
		..Default::default()
	}
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
