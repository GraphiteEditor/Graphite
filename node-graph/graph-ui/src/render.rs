use crate::look::{BACKDROP, painted, rounded_rect};
use crate::{GraphUi, nodes};
use core_types::list::{Item, List};
use core_types::{CloneVarArgs, Color, Context, Ctx, ExtractAll, ExtractVarArgs, OwnedContextImpl, Poll, ProtoNodeIdentifier};
use dyn_any::DynAny;
use glam::{DAffine2, DVec2};
use graph_craft::document::value::{RenderOutput, RenderOutputType};
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeId, NodeInput, NodeNetwork};
use graphene_application_io::{ExportFormat, RenderConfig};
use graphic_types::{Coverage, Vector};
use rendering::vello_scene_builder::VelloSceneBuilder;
use rendering::{Render, RenderParams, SvgRender, SvgRenderAttrs, SvgRenderOutput, format_transform_matrix};
use std::error::Error;
use std::sync::{Arc, Mutex, PoisonError};
use vello_gpu::kurbo::Affine;
use vello_gpu::peniko::ImageSampler;
use wgpu_executor::WgpuExecutor;

#[node_macro::node(category(""))]
fn no_artwork(_: impl Ctx) -> Poll<Item<RenderOutput>> {
	Poll::Pending
}

const WGPU_EXECUTOR: ProtoNodeIdentifier = ProtoNodeIdentifier::new("graphene_std::platform_application_io::WgpuExecutorNode");

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

pub fn wrap_network(network: &mut NodeNetwork) {
	let node = |identifier, inputs| DocumentNode {
		implementation: DocumentNodeImplementation::ProtoNode(identifier),
		inputs,
		..Default::default()
	};
	let first_band = network.nodes.keys().max().map_or(0, |id| id.0 + 1);
	let root = NodeId(first_band + BANDS.len() as u64);
	let mut inputs = vec![std::mem::replace(&mut network.exports[0], NodeInput::node(root, 0))];
	for (band, identifier) in (first_band..).map(NodeId).zip(BANDS) {
		network.nodes.insert(band, node(identifier, Vec::new()));
		inputs.push(NodeInput::node(band, 0));
	}
	inputs.push(NodeInput::scope(WGPU_EXECUTOR));
	inputs.push(NodeInput::scope(retained_artwork::IDENTIFIER));
	network.nodes.insert(root, node(render_graph_ui::IDENTIFIER, inputs));
}

#[node_macro::node(category(""))]
fn render_graph_ui(
	ctx: impl Ctx + ExtractVarArgs + ExtractAll + CloneVarArgs,
	artwork: impl Node<Context<'static>, Output = Item<RenderOutput>>,
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
	#[scope(WGPU_EXECUTOR)] executor: impl Node<Context<'static>, Output = Item<WgpuExecutor>>,
	#[scope(retained_artwork::IDENTIFIER)] retained_artwork: impl Node<Context<'static>, Output = Item<RetainedArtwork>>,
) -> Poll<Item<RenderOutput>> {
	let render_config = *render_config(&ctx);
	let generations = ctx.try_generations().cloned().unwrap_or_default();
	let context = OwnedContextImpl::from(ctx).into_context();

	let artwork = artwork.eval(&render_config.into_context_with_generations(generations));
	let store = retained_artwork.eval(&Context::default()).value().expect("The retained artwork store is always there").into_element();
	let mut retained = store.0.lock().unwrap_or_else(PoisonError::into_inner);
	let Some(graph_ui) = graph_ui(&context) else {
		if matches!(artwork, Poll::Final(_) | Poll::Partial(_) | Poll::Fallback(_)) {
			*retained = None;
		}
		return artwork;
	};
	let (fresh, outcome) = match artwork {
		Poll::Final(artwork) => (Some(artwork), Poll::Final(())),
		Poll::Partial(artwork) => (Some(artwork), Poll::Partial(())),
		Poll::Fallback(boxed) => (Some(boxed.0), Poll::Fallback(Box::new(((), boxed.1)))),
		Poll::Pending => (None, Poll::Partial(())),
		Poll::Error(error) => (None, Poll::Fallback(Box::new(((), *error)))),
	};
	if let Some(artwork) = fresh {
		*retained = Some(artwork);
	}
	let artwork = retained.as_ref().filter(|artwork| {
		matches!(
			(&artwork.element().data, render_config.export_format),
			(RenderOutputType::Texture(_), ExportFormat::Raster) | (RenderOutputType::Svg { .. }, ExportFormat::Svg)
		)
	});

	let viewport_size = render_config.viewport.resolution.as_dvec2() / render_config.scale;
	let opacity = if artwork.is_some() { graph_ui.config.fade_artwork as f32 } else { 1. };
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

	let data = match render_config.export_format {
		ExportFormat::Svg => svg_frame(&drawn, artwork, viewport_size),
		ExportFormat::Raster => {
			let executor = match executor.eval(&Context::default()) {
				Poll::Final(executor) | Poll::Partial(executor) => executor.into_element(),
				Poll::Fallback(boxed) => boxed.0.into_element(),
				Poll::Pending => return Poll::Pending,
				Poll::Error(error) => return Poll::Error(error),
			};
			match raster_frame(&executor, &drawn, artwork, &render_config) {
				Ok(data) => data,
				Err(error) => return Poll::error(error),
			}
		}
	};

	let metadata = artwork.map(|artwork| artwork.element().metadata.clone()).unwrap_or_default();
	outcome.map(|()| Item::new_from_element(RenderOutput { data, metadata }))
}

fn svg_frame(drawn: &[(List<Vector>, DAffine2)], artwork: Option<&Item<RenderOutput>>, viewport_size: DVec2) -> RenderOutputType {
	let mut render = SvgRender::new();
	for (band, transform) in drawn {
		let matrix = format_transform_matrix(*transform);
		let attributes = |attributes: &mut SvgRenderAttrs| {
			if !matrix.is_empty() {
				attributes.push("transform", matrix);
			}
		};
		render.parent_tag("g", attributes, |render| band.render_svg(render, &RenderParams::default()));
	}
	render.wrap_with_transform(DAffine2::IDENTITY, Some(viewport_size));
	let graph = SvgRenderOutput::from(render).svg;

	match artwork.map(|artwork| &artwork.element().data) {
		Some(RenderOutputType::Svg { svg, image_data }) => RenderOutputType::Svg {
			svg: format!("{svg}{graph}"),
			image_data: image_data.clone(),
		},
		_ => RenderOutputType::Svg { svg: graph, image_data: Vec::new() },
	}
}

fn raster_frame(
	executor: &WgpuExecutor,
	drawn: &[(List<Vector>, DAffine2)],
	artwork: Option<&Item<RenderOutput>>,
	render_config: &RenderConfig,
) -> Result<RenderOutputType, Box<dyn Error + Send + Sync>> {
	let viewport = DAffine2::from_scale(DVec2::splat(render_config.scale));
	let mut draw = |scene: &mut VelloSceneBuilder| {
		if let Some(RenderOutputType::Texture(texture)) = artwork.map(|artwork| &artwork.element().data) {
			scene.draw_texture(texture.clone(), ImageSampler::new(), Affine::IDENTITY);
		}
		for (band, transform) in drawn {
			band.render_to_vello(scene, viewport * *transform, &RenderParams::default());
		}
	};
	let texture = executor.render_vello(render_config.viewport.resolution, DAffine2::IDENTITY, &mut draw)?;
	Ok(RenderOutputType::Texture(texture))
}

#[node_macro::node(category(""), inject_scope)]
fn retained_artwork(_: impl Ctx, #[data] store: RetainedArtwork) -> Item<RetainedArtwork> {
	Item::new_from_element(store.clone())
}

#[derive(Clone, Debug, Default, DynAny)]
pub struct RetainedArtwork(Arc<Mutex<Option<Item<RenderOutput>>>>);

pub fn context(render_config: RenderConfig, graph_ui: Option<GraphUi>) -> OwnedContextImpl {
	let context = OwnedContextImpl::default();
	match graph_ui {
		Some(graph_ui) => context.with_varargs([Box::new(render_config), Box::new(graph_ui)]),
		None => context.with_vararg(Box::new(render_config)),
	}
}

pub(crate) fn render_config(ctx: &impl ExtractVarArgs) -> &RenderConfig {
	ctx.vararg(0).ok().and_then(|config| config.downcast_ref()).expect("The render config is the first vararg")
}

pub(crate) fn graph_ui(ctx: &impl ExtractVarArgs) -> Option<&GraphUi> {
	ctx.vararg(1).ok().and_then(|graph_ui| graph_ui.downcast_ref())
}
