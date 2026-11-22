use super::evaluated_output;
use crate::messages::frontend::FrontendMessage;
use atlas::ThumbnailAtlasLayout;
use glam::{DAffine2, DVec2, UVec2};
use graph_craft::application_io::PlatformEditorApi;
use graph_craft::document::{NodeId, NodeNetwork};
use graphene_std::application_io::{ApplicationIo, Texture};
use graphene_std::bounds::RenderBoundingBox;
use graphene_std::gradient::Gradient;
use graphene_std::list::{Item, List};
use graphene_std::ops::ConvertAsync;
#[cfg(all(target_family = "wasm", feature = "wasm"))]
use graphene_std::platform_application_io::canvas_utils::{Canvas, CanvasHandle};
use graphene_std::raster_types::{CPU, GPU, Raster};
use graphene_std::renderer::{Render, RenderParams, RenderSvgSegmentList, SvgRender};
use graphene_std::transform::{Footprint, RenderQuality};
use graphene_std::vector::Vector;
use graphene_std::{Artboard, Color, Context, ContextFeatures, Graphic};
use interpreted_executor::dynamic_executor::DynamicExecutor;
use interpreted_executor::layer_paths::layer_content;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

mod atlas;

/// The time thumbnails are evaluated at, so they don't depend on the playhead.
const THUMBNAIL_TIME: f64 = 0.;

/// The size in pixels of a thumbnail rendered at this device pixel ratio.
fn cell_size(scale: f64) -> UVec2 {
	/// The size thumbnails are shown at in the node graph, in CSS pixels.
	const CELL_SIZE: DVec2 = DVec2::new(72., 48.);

	(CELL_SIZE * scale).round().as_uvec2().max(UVec2::ONE)
}

/// The thumbnails of the layers, rendered from their content when it changes while they're in view.
#[derive(Default)]
pub struct Thumbnails {
	/// The proto node giving the content each layer's thumbnail shows.
	sources: HashMap<NodeId, NodeId>,
	/// The layers whose content changed since their thumbnail was last rendered.
	stale: HashSet<NodeId>,
	/// The layers whose thumbnails the frontend shows, the only ones rendered.
	in_view: HashSet<NodeId>,
	/// The device pixel ratio thumbnails are rendered at.
	scale: f64,
	#[cfg(all(target_family = "wasm", feature = "wasm"))]
	canvases: HashMap<NodeId, ThumbnailCanvases>,
	/// The layers whose thumbnails were sent to a page that can't show the runtime's canvases.
	#[cfg(not(all(target_family = "wasm", feature = "wasm")))]
	sent: HashSet<NodeId>,
}

/// A layer's thumbnail is shown in both the Layers panel and the node graph, but a canvas element can only be in one place.
#[cfg(all(target_family = "wasm", feature = "wasm"))]
struct ThumbnailCanvases {
	layers_panel: CanvasHandle,
	node_graph: CanvasHandle,
}

impl Thumbnails {
	pub fn new() -> Self {
		Self { scale: 1., ..Default::default() }
	}

	pub fn set_in_view(&mut self, layers: HashSet<NodeId>) {
		self.in_view = layers;
	}

	/// Whether the layer's thumbnail is waiting to be rendered.
	#[cfg(test)]
	pub fn is_stale(&self, layer: NodeId) -> bool {
		self.stale.contains(&layer)
	}

	/// Thumbnails rendered at another device pixel ratio are rendered again.
	pub fn set_scale(&mut self, scale: f64) {
		if scale != self.scale {
			self.stale.extend(self.sources.keys().copied());
		}
		self.scale = scale;
	}

	/// Finds the proto node giving each layer's content in the newly compiled graph, marking the layers whose proto node changed as stale.
	pub fn update_sources(&mut self, layer_paths: &[Vec<NodeId>], network: &NodeNetwork, executor: &DynamicExecutor) {
		// O(layers * depth)
		let sources: HashMap<NodeId, NodeId> = layer_paths
			.iter()
			.filter_map(|layer_path| Some((*layer_path.last()?, thumbnail_source(layer_path, network, executor)?)))
			.collect();

		// A proto node's ID hashes everything upstream of it, so a layer whose source kept its ID shows the same content
		let changed = sources.iter().filter(|(layer, source)| self.sources.get(layer) != Some(source));
		self.stale.extend(changed.map(|(layer, _)| *layer));
		self.stale.retain(|layer| sources.contains_key(layer));

		self.sources = sources;
	}

	/// Forgets the thumbnails of the layers with no content to show anymore, giving them back so the frontend can clear them.
	pub fn drop_sourceless(&mut self) -> Vec<NodeId> {
		let mut dropped = Vec::new();
		let sources = &self.sources;
		let mut keep_sourced = |layer: &NodeId| {
			let sourced = sources.contains_key(layer);
			if !sourced {
				dropped.push(*layer);
			}
			sourced
		};

		#[cfg(all(target_family = "wasm", feature = "wasm"))]
		self.canvases.retain(|layer, _| keep_sourced(layer));
		#[cfg(not(all(target_family = "wasm", feature = "wasm")))]
		self.sent.retain(|layer| keep_sourced(layer));

		dropped
	}

	/// Renders the thumbnails of the layers in view whose content changed since they were last rendered, keeping the others stale until they come into view.
	pub async fn render_stale(&mut self, executor: &DynamicExecutor, editor_api: &PlatformEditorApi, responses: &mut VecDeque<FrontendMessage>) {
		let mut contents = HashMap::new();
		for layer in std::mem::take(&mut self.stale) {
			let Some(&source) = self.sources.get(&layer) else { continue };

			if !self.in_view.contains(&layer) {
				self.stale.insert(layer);
				continue;
			}

			let Some(content) = ThumbnailContent::evaluate(executor, source) else {
				self.stale.insert(layer);
				continue;
			};
			contents.insert(layer, content);
		}
		if contents.is_empty() {
			return;
		}

		self.render_with_vello(&contents, editor_api, responses).await;
	}

	/// Renders the thumbnails into atlases with Vello and shows them.
	async fn render_with_vello(&mut self, contents: &HashMap<NodeId, ThumbnailContent>, editor_api: &PlatformEditorApi, responses: &mut VecDeque<FrontendMessage>) {
		let Some(executor) = editor_api.application_io.as_ref().and_then(|application_io| application_io.gpu_executor()) else {
			return;
		};

		let mut layers = contents.keys().copied().collect::<Vec<_>>();
		layers.sort_unstable();

		let max_texture_size = executor.context().device.limits().max_texture_dimension_2d;
		let cells_per_atlas = atlas::max_cells_per_atlas(cell_size(self.scale), max_texture_size);

		for layers in layers.chunks(cells_per_atlas) {
			let layout = ThumbnailAtlasLayout::new(layers.to_vec(), self.scale);

			let texture = executor.render_vello(layout.size(), DAffine2::IDENTITY, &mut |scene| atlas::draw_atlas(scene, &layout, contents));
			let texture = match texture {
				Ok(texture) => texture,
				Err(error) => {
					log::error!("Failed to render the thumbnails of layers {layers:?}: {error}");
					self.stale.extend(layers.iter().copied());
					continue;
				}
			};

			self.show_atlas(&layout, texture, executor, responses).await;
		}
	}

	/// Reads the atlas back and draws each cell into the canvases the layer's rows show.
	#[cfg(all(target_family = "wasm", feature = "wasm"))]
	async fn show_atlas(&mut self, layout: &ThumbnailAtlasLayout, atlas: Texture, executor: &wgpu_executor::WgpuExecutor, responses: &mut VecDeque<FrontendMessage>) {
		let raster = Raster::new_gpu(atlas).convert(Footprint::BOUNDLESS, executor.clone()).await;
		let (pixels, _, _) = raster.to_flat_u8();

		for (index, &layer) in layout.layers.iter().enumerate() {
			let cell_pixels = layout.cell_pixels(&pixels, index);
			let Ok(image) = web_sys::ImageData::new_with_u8_clamped_array_and_sh(wasm_bindgen::Clamped(&cell_pixels), layout.cell_size.x, layout.cell_size.y) else {
				continue;
			};

			let canvases = self.canvases(layer);
			for canvas in [&mut canvases.layers_panel, &mut canvases.node_graph] {
				canvas.set_resolution(layout.cell_size);
				let _ = canvas.context().put_image_data(&image, 0, 0);
			}

			responses.push_back(canvases.update_message(layer));
		}
	}

	/// Reads the atlas back for a page that can't show the runtime's canvases, like the desktop app's, and sends each layer its cell's pixels.
	#[cfg(not(all(target_family = "wasm", feature = "wasm")))]
	async fn show_atlas(&mut self, layout: &ThumbnailAtlasLayout, atlas: Texture, executor: &wgpu_executor::WgpuExecutor, responses: &mut VecDeque<FrontendMessage>) {
		let raster = Raster::new_gpu(atlas).convert(Footprint::BOUNDLESS, executor.clone()).await;
		let (pixels, _, _) = raster.to_flat_u8();

		for (index, &layer) in layout.layers.iter().enumerate() {
			self.sent.insert(layer);
			responses.push_back(FrontendMessage::UpdateNodeThumbnailPixels {
				id: layer,
				width: layout.cell_size.x,
				height: layout.cell_size.y,
				pixels: atlas::encode_transparent_runs(&layout.cell_pixels(&pixels, index)).into(),
			});
		}
	}

	#[cfg(all(target_family = "wasm", feature = "wasm"))]
	fn canvases(&mut self, layer: NodeId) -> &mut ThumbnailCanvases {
		self.canvases.entry(layer).or_insert_with(|| ThumbnailCanvases {
			layers_panel: CanvasHandle::new(),
			node_graph: CanvasHandle::new(),
		})
	}
}

#[cfg(all(target_family = "wasm", feature = "wasm"))]
impl ThumbnailCanvases {
	fn update_message(&mut self, layer: NodeId) -> FrontendMessage {
		FrontendMessage::UpdateNodeThumbnailCanvases {
			id: layer,
			layers_panel_canvas_id: self.layers_panel.id(),
			node_graph_canvas_id: self.node_graph.id(),
		}
	}
}

enum ThumbnailContent {
	Graphics(List<Graphic>),
	Artboards(List<Artboard>),
	Text(List<String>),
}

impl ThumbnailContent {
	/// Evaluates the content node again from its last context, normalized so thumbnails don't depend on the view or playhead.
	fn evaluate(executor: &DynamicExecutor, content_node: NodeId) -> Option<Self> {
		let tree = executor.tree();
		let context = tree.last_context(content_node).ok()??;

		// Replacing a feature the content doesn't read would only miss its caches
		let reads = tree.context_features(content_node);
		let footprint = reads.contains(ContextFeatures::FOOTPRINT).then_some(Footprint::BOUNDLESS);
		let time = reads.intersects(ContextFeatures::REAL_TIME | ContextFeatures::ANIMATION_TIME).then_some(THUMBNAIL_TIME);
		let normalized: Context = context.map(|context| Arc::new(context.with_replaced_footprint_and_time(footprint, time)));
		let output = evaluated_output(tree.evaluate(content_node, &normalized).ok()?)?;

		let content = if output.is::<List<Graphic>>() {
			Self::Graphics(*output.downcast().ok()?)
		} else if output.is::<Item<Graphic>>() {
			Self::Graphics(List::new_from_item(*output.downcast().ok()?))
		} else if output.is::<List<Artboard>>() {
			Self::Artboards(*output.downcast().ok()?)
		} else if output.is::<Item<Artboard>>() {
			Self::Artboards(List::new_from_item(*output.downcast().ok()?))
		} else if output.is::<List<String>>() {
			Self::Text(*output.downcast().ok()?)
		} else if output.is::<Item<String>>() {
			Self::Text(List::new_from_item(*output.downcast().ok()?))
		} else {
			// Any layer can show other graphical content as a group of its own, and shows nothing for content that isn't graphical
			Self::Graphics(into_graphic(output).map(List::new_from_element).unwrap_or_default())
		};
		Some(content)
	}

	fn graphic(&self) -> &dyn Render {
		match self {
			Self::Graphics(graphics) => graphics,
			Self::Artboards(artboards) => artboards,
			Self::Text(text) => text,
		}
	}

	/// The area the content covers once rendered, counting the effects drawn through maps and images only if they're `maps_resolved`, which the clipboard's SVG leaves out.
	fn content_bounds(&self, maps_resolved: bool) -> RenderBoundingBox {
		match self {
			// Text-aware bounds, which the `BoundingBox` trait can't give `Graphic::TextList` content, with the effects' reach
			Self::Graphics(graphics) => graphene_std::renderer::graphic_list_painted_bounds(graphics, maps_resolved),
			// The clipping rectangles rather than the content, since the renderer clips content to them
			Self::Artboards(artboards) => artboard_clip_bounds(artboards),
			Self::Text(text) => graphene_std::renderer::text_list_bounding_box(text, DAffine2::IDENTITY),
		}
	}

	/// The area the thumbnail shows, centered on the content at the thumbnail's aspect ratio.
	fn bounds(&self) -> [DVec2; 2] {
		let bounds = match self.content_bounds(true) {
			RenderBoundingBox::Rectangle(bounds) if (bounds[1] - bounds[0]) != DVec2::ZERO => bounds,
			_ => [DVec2::ZERO, DVec2::ONE],
		};
		expand_to_thumbnail_aspect(bounds)
	}
}

/// Renders what each of these document layers adds, painted in the given order, as one SVG framed to their combined bounds, or `None` if they render nothing.
pub fn layers_svg(layers: &[NodeId], network: &NodeNetwork, executor: &DynamicExecutor) -> Option<String> {
	let contents: Vec<ThumbnailContent> = layers
		.iter()
		.filter_map(|&layer| ThumbnailContent::evaluate(executor, thumbnail_source(&[layer], network, executor)?))
		.collect();

	let bounds = contents
		.iter()
		.filter_map(|content| match content.content_bounds(false) {
			RenderBoundingBox::Rectangle(bounds) if (bounds[1] - bounds[0]) != DVec2::ZERO => Some(bounds),
			_ => None,
		})
		.reduce(|[min, max], [other_min, other_max]| [min.min(other_min), max.max(other_max)])?;

	let render_params = RenderParams {
		footprint: Footprint::from_bounds(bounds, RenderQuality::Full),
		thumbnail: true,
		..Default::default()
	};
	let mut render = SvgRender::new();
	for content in &contents {
		content.graphic().render_svg(&mut render, &render_params);
	}
	if render.svg.is_empty() {
		return None;
	}
	render.format_svg(bounds[0], bounds[1]);

	Some(render.svg.to_svg_string())
}

/// The proto node giving the content a layer's thumbnail shows, which is only what the layer adds if it extends the stack below it.
fn thumbnail_source(layer_path: &[NodeId], network: &NodeNetwork, executor: &DynamicExecutor) -> Option<NodeId> {
	let source = layer_content(layer_path, network)?.source(network)?;

	executor.tree().source_map().get(source.as_slice()).map(|(id, _)| *id)
}

/// Converts the other kinds of graphical content into a graphic.
fn into_graphic(output: graphene_std::registry::Any) -> Option<Graphic> {
	macro_rules! downcast_into_graphic {
		($($content:ty),*) => {$(
			if output.is::<$content>() {
				return output.downcast::<$content>().ok().map(|content| Graphic::from(*content));
			}
		)*};
	}

	downcast_into_graphic!(
		List<Vector>,
		Item<Vector>,
		List<Raster<CPU>>,
		Item<Raster<CPU>>,
		List<Raster<GPU>>,
		Item<Raster<GPU>>,
		List<Color>,
		Item<Color>,
		List<Gradient>,
		Item<Gradient>
	);
	None
}

/// Returns the union of the artboards' clipping rectangles, used as the thumbnail bounds for an artboard layer so the
/// framing matches what's actually visible after clipping rather than the unclipped content extents.
fn artboard_clip_bounds(artboards: &List<Artboard>) -> RenderBoundingBox {
	let mut combined: Option<[DVec2; 2]> = None;
	for index in 0..artboards.len() {
		let location: DVec2 = artboards.attribute_cloned_or_default(graphene_std::ATTR_LOCATION, index);
		let dimensions: DVec2 = artboards.attribute_cloned_or_default(graphene_std::ATTR_DIMENSIONS, index);
		let bounds = [location, location + dimensions];
		combined = Some(match combined {
			Some(existing) => [existing[0].min(bounds[0]), existing[1].max(bounds[1])],
			None => bounds,
		});
	}
	match combined {
		Some(bounds) => RenderBoundingBox::Rectangle(bounds),
		None => RenderBoundingBox::None,
	}
}

/// Expands an AABB outward (centered) to match the Layers panel thumbnail's 3:2 aspect ratio, padding the smaller axis
/// so the input's extent is always preserved.
fn expand_to_thumbnail_aspect(bounds: [DVec2; 2]) -> [DVec2; 2] {
	const THUMBNAIL_ASPECT_RATIO: f64 = 1.5;

	let size = bounds[1] - bounds[0];
	let center = (bounds[0] + bounds[1]) / 2.;
	let (width, height) = if size.x >= size.y * THUMBNAIL_ASPECT_RATIO {
		(size.x, size.x / THUMBNAIL_ASPECT_RATIO)
	} else {
		(size.y * THUMBNAIL_ASPECT_RATIO, size.y)
	};
	let half = DVec2::new(width, height) / 2.;
	[center - half, center + half]
}

#[cfg(test)]
mod tests {
	use crate::messages::portfolio::document::utility_types::network_interface::{FlowType, shipped_node_id};
	use crate::test_utils::test_prelude::*;
	use glam::UVec2;
	use graph_craft::document::NodeId;
	use graphene_std::application_io::TimingInformation;
	use interpreted_executor::layer_paths::layer_path_writer_id;

	async fn compile_and_run(editor: &mut EditorTestUtils) {
		let portfolio = editor.portfolio_message_handler_mut();
		let document_id = portfolio.active_document_id.unwrap();
		let (executor, documents) = (&mut portfolio.executor, &mut portfolio.documents);
		let document = documents.get_mut(&document_id).unwrap();
		executor
			.submit_node_graph_evaluation(document, document_id, UVec2::ONE, 1., TimingInformation::default(), Vec::new(), true, DVec2::ZERO)
			.unwrap();
		editor.runtime.update();
		editor.runtime.run().await;
		editor.dispatch(Message::Wake);
	}

	fn compiled_id(editor: &EditorTestUtils, path: &[NodeId]) -> Option<NodeId> {
		editor.runtime.executor.tree().source_map().get(path).map(|(id, _)| *id)
	}

	#[tokio::test]
	async fn a_merge_layer_shows_its_content_and_another_layer_its_output() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.draw_rect(0., 0., 10., 10.).await;
		let layer = editor.get_selected_layer().await.expect("The drawn rectangle should be selected").to_node();

		compile_and_run(&mut editor).await;
		let merge_content = compiled_id(&editor, &[layer, layer_path_writer_id(shipped_node_id("Merge", NodeId(2)))]);
		assert!(merge_content.is_some(), "The Merge layer's content should be compiled");
		assert_eq!(
			editor.runtime.thumbnails.sources.get(&layer).copied(),
			merge_content,
			"A Merge layer shows only the content it adds to the stack"
		);

		// The node feeding the layer's content, shown as a layer itself
		let network_interface = &mut editor.active_document_mut().network_interface;
		let content_node = network_interface
			.upstream_flow_back_from_nodes(vec![layer], &[], FlowType::HorizontalFlow)
			.nth(1)
			.expect("The layer should have a content chain");
		network_interface.set_to_node_or_layer(&content_node, &[], true);

		compile_and_run(&mut editor).await;
		let content_output = compiled_id(&editor, &[content_node]);
		assert!(content_output.is_some(), "The content node should be compiled");
		assert_eq!(editor.runtime.thumbnails.sources.get(&content_node).copied(), content_output, "Any other layer shows its output");
	}
}
