mod render;
mod timing;

use core_types::color::SRGBA8;
use core_types::list::Item;
use core_types::uuid::NodeId;
use core_types::{Context, Ctx, ExtractVarArgs, ProtoNodeIdentifier};
use glam::{DAffine2, DVec2, IVec2, UVec2};
use graph_craft::application_io::{PlatformApplicationIo, PlatformEditorApi};
use graph_craft::document::value::{RenderOutput, RenderOutputType, TaggedValue};
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeInput, NodeNetwork};
use graph_craft::item;
use graph_craft::proto::ProtoNetwork;
use graph_ui::card::{Card, Connector, DataType, Highlight, Selection};
use graph_ui::error_badge::ErrorBadge;
use graph_ui::layer::Layer;
use graph_ui::network_connector::{NetworkConnector, NetworkConnectorKind};
use graph_ui::wire::{Wire, WireKey};
use graph_ui::{GraphUi, GraphUiConfig};
use graphene_std::application_io::RenderConfig;
use graphene_std::platform_application_io;
use graphic_types::Graphic;
use graphic_types::raster_types::Raster;
use interpreted_executor::dynamic_executor::DynamicExecutor;
use rendering::to_peniko::ToPenikoColor;
use rendering::vello_scene_builder::VelloSceneBuilder;
use std::cell::Cell;
use std::sync::Arc;
use vello_gpu::kurbo::{Affine, BezPath, Rect};
use vello_gpu::peniko::Fill;
use wgpu_executor::WgpuExecutor;

const ARTWORK: SRGBA8 = SRGBA8::new_opaque(0x30, 0x60, 0x90);

thread_local! {
	static ARTWORK_RENDERS: Cell<usize> = const { Cell::new(0) };
}

#[node_macro::node(category(""), memoize)]
fn artwork(ctx: impl Ctx + ExtractVarArgs, #[scope(platform_application_io::wgpu_executor::IDENTIFIER)] executor: Item<WgpuExecutor>) -> Item<RenderOutput> {
	ARTWORK_RENDERS.set(ARTWORK_RENDERS.get() + 1);
	rendered_artwork(&ctx, executor.element())
}

fn rendered_artwork(ctx: &impl ExtractVarArgs, executor: &WgpuExecutor) -> Item<RenderOutput> {
	let config: &RenderConfig = ctx.vararg(0).unwrap().downcast_ref().unwrap();
	let mut fill = |scene: &mut VelloSceneBuilder| scene.fill_viewport(ARTWORK.to_peniko_color(), None);
	let texture = executor.render_vello(config.viewport.resolution, DAffine2::IDENTITY, &mut fill).unwrap();
	Item::new_from_element(RenderOutput {
		data: RenderOutputType::Texture(texture),
		metadata: Default::default(),
	})
}

fn gpu() -> WgpuExecutor {
	futures::executor::block_on(WgpuExecutor::new()).expect("rendering needs a GPU adapter, such as lavapipe through `VK_ADD_DRIVER_FILES`")
}

fn wgpu_executor() -> NodeInput {
	NodeInput::scope(platform_application_io::wgpu_executor::IDENTIFIER)
}

fn node(identifier: ProtoNodeIdentifier, inputs: Vec<NodeInput>) -> DocumentNode {
	DocumentNode {
		implementation: DocumentNodeImplementation::ProtoNode(identifier),
		inputs,
		..Default::default()
	}
}

fn network(gpu: &WgpuExecutor, nodes: impl IntoIterator<Item = DocumentNode>) -> NodeNetwork {
	let editor_api = Arc::new(PlatformEditorApi {
		application_io: Some(Arc::new(PlatformApplicationIo::new_with_executor(gpu.clone()))),
	});
	let editor_api = node(graphene_std::ops::passthrough::IDENTIFIER, vec![NodeInput::value(TaggedValue::EditorApi(editor_api), false)]);
	let nodes: Vec<DocumentNode> = [editor_api].into_iter().chain(nodes).collect();
	NodeNetwork {
		exports: vec![NodeInput::node(NodeId(nodes.len() as u64 - 1), 0)],
		nodes: nodes.into_iter().enumerate().map(|(id, node)| (NodeId(id as u64), node)).collect(),
		scope_injections: [("editor-api".to_string(), (NodeId(0), item!(Arc<PlatformEditorApi>)))].into_iter().collect(),
		..Default::default()
	}
}

fn compiled(mut network: NodeNetwork) -> ProtoNetwork {
	preprocessor::Preprocessor::new().preprocess(&mut network, &|_| None).unwrap();
	graph_craft::util::compile(network)
}

fn executor(network: NodeNetwork) -> DynamicExecutor {
	DynamicExecutor::new(compiled(network)).unwrap()
}

fn config(view: DAffine2, fade_artwork: f64) -> GraphUiConfig {
	GraphUiConfig {
		view,
		fade_artwork,
		wire_in_progress: None,
		reorder_bar: None,
		box_selection: None,
	}
}

fn context(render_config: RenderConfig, graph_ui: Option<GraphUi>) -> Context<'static> {
	graph_ui::render::context(render_config, graph_ui).into_context()
}

fn thumbnail(gpu: &WgpuExecutor, painted: Rect, color: SRGBA8) -> Graphic {
	let mut paint = |scene: &mut VelloSceneBuilder| scene.fill(Fill::NonZero, Affine::IDENTITY, color.to_peniko_color(), None, &painted);
	let texture = gpu.render_vello(UVec2::new(72, 48), DAffine2::IDENTITY, &mut paint).unwrap();
	Graphic::RasterGPU(Item::new_from_element(Raster::new_gpu(texture)))
}

fn card(key: u64, (x, y): (i32, i32), name: &str) -> Card {
	Card {
		key: NodeId(key),
		cell: IVec2::new(x, y),
		name: name.to_string(),
		icon: "NodeCustomNode".to_string(),
		primary_input: None,
		primary_output: None,
		secondary_inputs: Vec::new(),
		secondary_outputs: Vec::new(),
		selection: Selection::None,
		visible: true,
		previewed: false,
		glide: DVec2::ZERO,
	}
}

fn layer(key: u64, (x, y): (i32, i32), name: &str) -> Layer {
	Layer {
		key: NodeId(key),
		cell: IVec2::new(x, y),
		name: name.to_string(),
		renaming: false,
		width: 8,
		chain_width: 0,
		has_left_input_wire: false,
		primary_input: None,
		primary_output: None,
		secondary_input: None,
		primary_input_connected_to_layer: false,
		primary_output_connected_to_layer: false,
		visible: true,
		locked: false,
		selection: Selection::None,
		previewed: false,
		hovered_button: None,
		hovered_button_clicked: false,
		glide: DVec2::ZERO,
	}
}

fn wire(key: WireKey, path: BezPath, data_type: DataType) -> Wire {
	Wire {
		key,
		path,
		data_type,
		thick: false,
		center_line: BezPath::new(),
		list: false,
		insertion_target: false,
	}
}

fn network_connector(kind: NetworkConnectorKind, index: usize, (x, y): (i32, i32), connector: Option<Connector>) -> NetworkConnector {
	NetworkConnector {
		kind,
		index,
		position: IVec2::new(x, y),
		connector,
		add_highlight: Highlight::None,
		editable: false,
		renaming: false,
		glide: DVec2::ZERO,
	}
}

fn badge(key: u64, (x, y): (i32, i32), text: &str) -> ErrorBadge {
	ErrorBadge {
		key: NodeId(key),
		cell: IVec2::new(x, y),
		on_layer: false,
		text: text.to_string(),
		hovered: false,
		glide: DVec2::ZERO,
	}
}

fn connector(name: &str, data_type: DataType) -> Connector {
	Connector {
		name: name.to_string(),
		data_type,
		connected: false,
		highlight: Highlight::None,
	}
}
