use super::{evaluated_output, monitor_parent_node, records_vector};
use crate::messages::frontend::FrontendMessage;
#[cfg(feature = "gpu")]
use atlas::ThumbnailAtlasLayout;
use glam::{DAffine2, DVec2, UVec2};
use graph_craft::application_io::PlatformEditorApi;
use graph_craft::document::NodeId;
#[cfg(feature = "gpu")]
use graphene_std::application_io::{ApplicationIo, Texture};
use graphene_std::bounds::RenderBoundingBox;
use graphene_std::list::{Item, List};
#[cfg(feature = "gpu")]
use graphene_std::ops::ConvertAsync;
#[cfg(all(target_family = "wasm", feature = "wasm"))]
use graphene_std::platform_application_io::canvas_utils::{Canvas, CanvasHandle};
#[cfg(feature = "gpu")]
use graphene_std::raster_types::Raster;
use graphene_std::renderer::{Render, RenderParams, RenderSvgSegmentList, SvgRender};
use graphene_std::transform::{Footprint, RenderQuality};
use graphene_std::{Artboard, Context, ContextFeatures, Graphic};
use interpreted_executor::dynamic_executor::DynamicExecutor;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

#[cfg(feature = "gpu")]
mod atlas;

/// The time thumbnails are evaluated at, so they don't depend on the playhead.
const THUMBNAIL_TIME: f64 = 0.;

/// The size in pixels of a thumbnail rendered at this device pixel ratio.
#[cfg_attr(
	not(any(feature = "gpu", all(target_family = "wasm", feature = "wasm"))),
	expect(dead_code, reason = "Only thumbnails rendered with Vello or drawn into canvases have a fixed size")
)]
fn cell_size(scale: f64) -> UVec2 {
	/// The size thumbnails are shown at in the node graph, in CSS pixels.
	const CELL_SIZE: DVec2 = DVec2::new(72., 48.);

	(CELL_SIZE * scale).round().as_uvec2().max(UVec2::ONE)
}

/// The thumbnails of the layers, rendered from the content their monitor nodes watch when it changes while they're in view.
#[derive(Default)]
pub struct Thumbnails {
	/// The monitor nodes whose content changed since their layer's thumbnail was last rendered.
	stale: HashSet<Vec<NodeId>>,
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

	/// Thumbnails rendered at another device pixel ratio are rendered again.
	pub fn set_scale(&mut self, scale: f64, monitor_nodes: &[(Vec<NodeId>, NodeId)]) {
		if scale != self.scale {
			self.stale.extend(monitor_nodes.iter().map(|(path, _)| path.clone()));
		}
		self.scale = scale;
	}

	/// Marks the thumbnails of the monitor nodes whose proto node ID changed as stale.
	pub fn update_monitors(&mut self, previous_monitor_ids: &HashMap<Vec<NodeId>, NodeId>, monitor_nodes: &[(Vec<NodeId>, NodeId)]) {
		// A proto node's ID hashes everything upstream of it, so a monitor node whose ID stayed the same records the same output
		let changed_monitors = monitor_nodes.iter().filter(|(path, id)| previous_monitor_ids.get(path) != Some(id));
		self.stale.extend(changed_monitors.map(|(path, _)| path.clone()));

		let current_monitors: HashSet<&Vec<NodeId>> = monitor_nodes.iter().map(|(path, _)| path).collect();
		self.stale.retain(|path| current_monitors.contains(path));
	}

	/// Forgets the thumbnails of the layers no longer monitored, giving them back so the frontend can clear them.
	pub fn drop_unmonitored(&mut self, monitor_nodes: &[(Vec<NodeId>, NodeId)]) -> Vec<NodeId> {
		// TODO: Consider optimizing this since it's currently O(m*n^2), with a sort it could be made O(m * n*log(n))
		let mut dropped = Vec::new();
		let mut keep_monitored = |id: &NodeId| {
			let monitored = monitor_nodes.iter().any(|(monitor_node_path, _)| monitor_node_path.contains(id));
			if !monitored && !dropped.contains(id) {
				dropped.push(*id);
			}
			monitored
		};

		#[cfg(all(target_family = "wasm", feature = "wasm"))]
		self.canvases.retain(|id, _| keep_monitored(id));
		#[cfg(not(all(target_family = "wasm", feature = "wasm")))]
		self.sent.retain(|id| keep_monitored(id));

		dropped
	}

	/// Renders the thumbnails of the layers in view whose content changed since they were last rendered, keeping the others stale until they come into view.
	pub async fn render_stale(&mut self, executor: &DynamicExecutor, editor_api: &PlatformEditorApi, responses: &mut VecDeque<FrontendMessage>) {
		let mut contents = HashMap::new();
		for monitor_node_path in std::mem::take(&mut self.stale) {
			let Some(layer) = monitor_parent_node(&monitor_node_path) else { continue };

			if records_vector(executor, &monitor_node_path) {
				continue;
			}
			if !self.in_view.contains(&layer) {
				self.stale.insert(monitor_node_path);
				continue;
			}

			let Some(content) = ThumbnailContent::evaluate(executor, &monitor_node_path) else {
				self.stale.insert(monitor_node_path);
				continue;
			};
			contents.insert(layer, content);
		}
		if contents.is_empty() {
			return;
		}

		if !self.render_with_vello(&contents, editor_api, responses).await {
			for (layer, content) in &contents {
				self.render_svg(*layer, content, responses).await;
			}
		}
	}

	/// Renders the thumbnails into atlases with Vello and shows them, giving false if Vello is unavailable.
	#[cfg(feature = "gpu")]
	async fn render_with_vello(&mut self, contents: &HashMap<NodeId, ThumbnailContent>, editor_api: &PlatformEditorApi, responses: &mut VecDeque<FrontendMessage>) -> bool {
		let Some(executor) = editor_api.application_io.as_ref().and_then(|application_io| application_io.gpu_executor()) else {
			return false;
		};

		let mut layers = contents.keys().copied().collect::<Vec<_>>();
		layers.sort_unstable();

		let max_texture_size = executor.context().device.limits().max_texture_dimension_2d;
		let cells_per_atlas = atlas::max_cells_per_atlas(cell_size(self.scale), max_texture_size);

		for layers in layers.chunks(cells_per_atlas) {
			let layout = ThumbnailAtlasLayout::new(layers.to_vec(), self.scale);

			let texture = executor.vello_scene(layout.size(), DAffine2::IDENTITY).and_then(|mut scene| {
				atlas::draw_atlas(&mut scene, &layout, contents);
				executor.render_vello_scene(scene)
			});
			let texture = match texture {
				Ok(texture) => texture,
				Err(error) => {
					log::error!("Failed to render the thumbnails of layers {layers:?}: {error}");
					return false;
				}
			};

			self.show_atlas(&layout, texture, executor, responses).await;
		}

		true
	}

	#[cfg(not(feature = "gpu"))]
	async fn render_with_vello(&mut self, _contents: &HashMap<NodeId, ThumbnailContent>, _editor_api: &PlatformEditorApi, _responses: &mut VecDeque<FrontendMessage>) -> bool {
		false
	}

	/// Reads the atlas back and draws each cell into the canvases the layer's rows show.
	#[cfg(all(target_family = "wasm", feature = "gpu", feature = "wasm"))]
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
	#[cfg(all(feature = "gpu", not(all(target_family = "wasm", feature = "wasm"))))]
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

	async fn render_svg(&mut self, layer: NodeId, content: &ThumbnailContent, responses: &mut VecDeque<FrontendMessage>) {
		let bounds = content.bounds();
		let footprint = Footprint {
			transform: DAffine2::from_translation(bounds[0]),
			resolution: (bounds[1] - bounds[0]).abs().as_uvec2(),
			quality: RenderQuality::Full,
		};
		let render_params = RenderParams {
			footprint,
			thumbnail: true,
			..Default::default()
		};
		let mut render = SvgRender::new();
		content.graphic().render_svg(&mut render, &render_params);
		render.format_svg(bounds[0], bounds[1]);

		self.show_svg(layer, render.svg.to_svg_string(), responses).await;
	}

	/// Draws the SVG into the canvases the layer's rows show, so the browser rasterizes it once rather than keeping it in the page.
	#[cfg(all(target_family = "wasm", feature = "wasm"))]
	async fn show_svg(&mut self, layer: NodeId, svg: String, responses: &mut VecDeque<FrontendMessage>) {
		use base64::Engine;

		let size = cell_size(self.scale);

		// An SVG drawn as an image needs an intrinsic size, which the thumbnail's only gives as a view box
		let svg = svg.replacen("<svg ", &format!("<svg width=\"{}\" height=\"{}\" ", size.x, size.y), 1);
		let mut url = String::from("data:image/svg+xml;base64,");
		base64::engine::general_purpose::STANDARD.encode_string(svg, &mut url);

		let Ok(image) = web_sys::HtmlImageElement::new() else { return };
		image.set_src(&url);
		if let Err(error) = wasm_bindgen_futures::JsFuture::from(image.decode()).await {
			log::error!("Failed to decode the thumbnail of layer {layer:?}: {error:?}");
			return;
		}

		let canvases = self.canvases(layer);
		for canvas in [&mut canvases.layers_panel, &mut canvases.node_graph] {
			canvas.set_resolution(size);
			let context = canvas.context();
			context.clear_rect(0., 0., size.x as f64, size.y as f64);
			let _ = context.draw_image_with_html_image_element_and_dw_and_dh(&image, 0., 0., size.x as f64, size.y as f64);
		}

		responses.push_back(canvases.update_message(layer));
	}

	#[cfg(not(all(target_family = "wasm", feature = "wasm")))]
	async fn show_svg(&mut self, layer: NodeId, svg: String, responses: &mut VecDeque<FrontendMessage>) {
		self.sent.insert(layer);
		responses.push_back(FrontendMessage::UpdateNodeThumbnail { id: layer, value: svg });
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
	/// Evaluates the content a monitor node watches again from its last context, normalized so thumbnails don't depend on the view or playhead.
	fn evaluate(executor: &DynamicExecutor, monitor_node_path: &[NodeId]) -> Option<Self> {
		let tree = executor.tree();
		let content_node = tree.input_node(monitor_node_path, 0).ok()?;
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
			log::warn!("Failed to downcast the content of monitor node {monitor_node_path:?} for its thumbnail");
			return None;
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

	/// The area the thumbnail shows, centered on the content at the thumbnail's aspect ratio.
	fn bounds(&self) -> [DVec2; 2] {
		let bounds = match self {
			// Text-aware bounds, since the `BoundingBox` trait can't lay out `Graphic::TextList` content
			Self::Graphics(graphics) => graphene_std::renderer::graphic_list_bounding_box(graphics, DAffine2::IDENTITY),
			// The clipping rectangles rather than the content, since the renderer clips content to them
			Self::Artboards(artboards) => artboard_clip_bounds(artboards),
			Self::Text(text) => graphene_std::renderer::text_list_bounding_box(text, DAffine2::IDENTITY),
		};

		let bounds = match bounds {
			RenderBoundingBox::Rectangle(bounds) if (bounds[1] - bounds[0]) != DVec2::ZERO => bounds,
			_ => [DVec2::ZERO, DVec2::ONE],
		};
		expand_to_thumbnail_aspect(bounds)
	}
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
