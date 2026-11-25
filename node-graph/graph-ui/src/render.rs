use crate::look::{BACKDROP, painted, rounded_rect};
use crate::{GraphUi, nodes};
use core_types::list::{Item, List};
use core_types::{CloneVarArgs, Color, Context, Ctx, ExtractAll, ExtractVarArgs, OwnedContextImpl, Poll, ProtoNodeIdentifier};
use glam::{DAffine2, DVec2};
use graph_craft::document::value::{RenderOutput, RenderOutputType};
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeId, NodeInput, NodeNetwork};
use graphene_application_io::RenderConfig;
use graphic_types::{Coverage, Vector};
use rendering::vello_scene_builder::VelloSceneBuilder;
use rendering::{Render, RenderParams};
use std::error::Error;
use wgpu_executor::WgpuExecutor;

const BANDS: [ProtoNodeIdentifier; 13] = [
	nodes::draw_grid::IDENTIFIER,
	nodes::draw_thick_wires::IDENTIFIER,
	nodes::draw_layers::IDENTIFIER,
	nodes::draw_thumbnails::IDENTIFIER,
	nodes::draw_wires::IDENTIFIER,
	nodes::draw_wire_in_progress::IDENTIFIER,
	nodes::draw_cards::IDENTIFIER,
	nodes::draw_dots::IDENTIFIER,
	nodes::draw_raised::IDENTIFIER,
	nodes::draw_errors::IDENTIFIER,
	nodes::draw_network_connectors::IDENTIFIER,
	nodes::draw_reorder_bar::IDENTIFIER,
	nodes::draw_box_selection::IDENTIFIER,
];

pub fn network() -> NodeNetwork {
	let node = |identifier, inputs| DocumentNode {
		implementation: DocumentNodeImplementation::ProtoNode(identifier),
		inputs,
		..Default::default()
	};
	let mut network = NodeNetwork::default();
	let root = NodeId(BANDS.len() as u64);
	let mut inputs = Vec::new();
	for (band, identifier) in (0..).map(NodeId).zip(BANDS) {
		network.nodes.insert(band, node(identifier, Vec::new()));
		inputs.push(NodeInput::node(band, 0));
	}
	inputs.push(NodeInput::scope(graphene_std::platform_application_io::wgpu_executor::IDENTIFIER));
	network.nodes.insert(root, node(render_graph_ui::IDENTIFIER, inputs));
	network.exports.push(NodeInput::node(root, 0));
	network
}

#[node_macro::node(category(""))]
fn render_graph_ui(
	ctx: impl Ctx + ExtractVarArgs + ExtractAll + CloneVarArgs,
	grid: impl Node<Context<'static>, Output = List<Vector>>,
	thick_wires: impl Node<Context<'static>, Output = List<Vector>>,
	layers: impl Node<Context<'static>, Output = List<Vector>>,
	thumbnails: impl Node<Context<'static>, Output = List<Vector>>,
	wires: impl Node<Context<'static>, Output = List<Vector>>,
	wire_in_progress: impl Node<Context<'static>, Output = List<Vector>>,
	cards: impl Node<Context<'static>, Output = List<Vector>>,
	dots: impl Node<Context<'static>, Output = List<Vector>>,
	raised: impl Node<Context<'static>, Output = List<Vector>>,
	errors: impl Node<Context<'static>, Output = List<Vector>>,
	imports_exports: impl Node<Context<'static>, Output = List<Vector>>,
	reorder_bar: impl Node<Context<'static>, Output = List<Vector>>,
	box_selection: impl Node<Context<'static>, Output = List<Vector>>,
	#[scope(graphene_std::platform_application_io::wgpu_executor::IDENTIFIER)] executor: impl Node<Context<'static>, Output = Item<WgpuExecutor>>,
) -> Poll<Item<RenderOutput>> {
	let render_config = *render_config(&ctx);
	let context = OwnedContextImpl::from(ctx).into_context();
	let graph_ui = graph_ui(&context);

	let viewport_size = render_config.viewport.resolution.as_dvec2() / render_config.scale;
	let opacity = graph_ui.config.fade_artwork as f32;
	let backdrop = painted(rounded_rect(DVec2::ZERO, viewport_size, [0.; 4]), Coverage::new_fill(), Color::from(BACKDROP).with_alpha(opacity));
	let mut drawn = vec![(List::new_from_item(backdrop), DAffine2::IDENTITY)];
	let scene_context = OwnedContextImpl::default().with_vararg(Box::new(graph_ui.scene.clone())).into_context();
	let bands = [
		(grid.eval(&context), DAffine2::IDENTITY),
		(thick_wires.eval(&scene_context), graph_ui.config.view),
		(layers.eval(&scene_context), graph_ui.config.view),
		(thumbnails.eval(&context), graph_ui.config.view),
		(wires.eval(&scene_context), graph_ui.config.view),
		(wire_in_progress.eval(&context), graph_ui.config.view),
		(cards.eval(&scene_context), graph_ui.config.view),
		(dots.eval(&scene_context), graph_ui.config.view),
		(raised.eval(&scene_context), graph_ui.config.view),
		(errors.eval(&scene_context), graph_ui.config.view),
		(imports_exports.eval(&scene_context), graph_ui.config.view),
		(reorder_bar.eval(&context), graph_ui.config.view),
		(box_selection.eval(&context), DAffine2::IDENTITY),
	];
	for (band, transform) in bands {
		let band = match band {
			Poll::Final(band) | Poll::Partial(band) => band,
			Poll::Fallback(boxed) => boxed.0,
			Poll::Pending => return Poll::Pending,
			Poll::Error(error) => return Poll::Error(error),
		};
		drawn.push((band, transform));
	}

	let executor = match executor.eval(&Context::default()) {
		Poll::Final(executor) | Poll::Partial(executor) => executor.into_element(),
		Poll::Fallback(boxed) => boxed.0.into_element(),
		Poll::Pending => return Poll::Pending,
		Poll::Error(error) => return Poll::Error(error),
	};
	let data = match raster_frame(&executor, &drawn, &render_config) {
		Ok(data) => data,
		Err(error) => return Poll::error(error),
	};

	let metadata = Default::default();
	Poll::Final(Item::new_from_element(RenderOutput { data, metadata }))
}

fn raster_frame(executor: &WgpuExecutor, drawn: &[(List<Vector>, DAffine2)], render_config: &RenderConfig) -> Result<RenderOutputType, Box<dyn Error + Send + Sync>> {
	let viewport = DAffine2::from_scale(DVec2::splat(render_config.scale));
	let mut draw = |scene: &mut VelloSceneBuilder| {
		for (band, transform) in drawn {
			band.render_to_vello(scene, viewport * *transform, &RenderParams::default());
		}
	};
	let texture = executor.render_vello(render_config.viewport.resolution, DAffine2::IDENTITY, &mut draw)?;
	Ok(RenderOutputType::Texture(texture))
}

pub fn context(render_config: RenderConfig, graph_ui: GraphUi) -> OwnedContextImpl {
	OwnedContextImpl::default().with_varargs([Box::new(render_config), Box::new(graph_ui)])
}

pub(crate) fn render_config(ctx: &impl ExtractVarArgs) -> &RenderConfig {
	ctx.vararg(0).ok().and_then(|config| config.downcast_ref()).expect("The render config is the first vararg")
}

pub(crate) fn graph_ui(ctx: &impl ExtractVarArgs) -> &GraphUi {
	ctx.vararg(1).ok().and_then(|graph_ui| graph_ui.downcast_ref()).expect("The graph UI is the second vararg")
}
