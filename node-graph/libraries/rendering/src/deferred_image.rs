//! What SVG can't express, which the GPU renders as images for an SVG to take, once the SVG is drawn.
//! An effect whose coverage SVG filters can't work out, like the shadows and glows shaped for PSD interop, goes in its filter as a map of that coverage, while the content keeps its own markup.
//! Content with an effect no filter can carry out at all is drawn as an image of it in place of its markup.
//! Placeholders stand in for each until it's resolved, leaving an SVG that never is short of those effects.

use crate::renderer::{CanvasImage, Render, RenderOutputType, RenderParams, RenderSvgSegmentList, SvgRender, render_raster_cpu_image_svg};
use crate::vello_scene_builder::{MAX_EFFECTS_AREA_PER_VIEWPORT_AREA, VelloSceneBuilder};
use core_types::Color;
use core_types::blending::BlendMode;
use core_types::color::SRGBA8;
use core_types::math::quad::Quad;
use core_types::transform::Footprint;
use glam::{DAffine2, DVec2, UVec2};
use graphic_types::Graphic;
use graphic_types::effects::EffectCapture;
use graphic_types::raster_types::{CPU, Raster};

/// The most pixels across or down a map or image may have.
pub const MAX_DEFERRED_IMAGE_PIXELS: u32 = 8192;

/// The fraction of full resolution an image of an `area` of a view's pixels gets, with at most `max_pixels` across or down and the same budget of the `view_area` as an effects group in Vello.
pub(crate) fn budgeted_resolution(area: DVec2, view_area: f64, max_pixels: f64) -> f64 {
	let within_texture = max_pixels / area.max_element();
	let within_budget = (MAX_EFFECTS_AREA_PER_VIEWPORT_AREA * view_area / (area.x * area.y)).sqrt();
	within_texture.min(within_budget).min(1.)
}

/// The image pixels covering the part of a user-space region in a view that `document_to_view` maps the document into, spanning `view_size` pixels grown by `margin` in document pixels, with at most `max_pixels` across or down and a budget of the view's area.
/// Returns the transform placing the image's unit square in user space, the transform mapping the document into its pixels, and how many pixels it has, or `None` if none of it lies in view.
fn image_layout(user_to_document: DAffine2, bounds: [DVec2; 2], margin: f64, document_to_view: DAffine2, view_size: DVec2, max_pixels: u32) -> Option<(DAffine2, DAffine2, UVec2)> {
	let user_to_view = document_to_view * user_to_document;
	if !user_to_view.matrix2.determinant().is_normal() || !user_to_view.is_finite() {
		return None;
	}

	// The view's pixels the region reaches into, whole ones so the image's pixels line up with the view's
	let margin = margin * document_to_view.matrix2.determinant().abs().sqrt();
	let [min, max] = (user_to_view * Quad::from_box(bounds)).bounding_box();
	let (min, max) = (min.max(DVec2::splat(-margin)).floor(), max.min(view_size + margin).ceil());
	let area = max - min;
	if !(area.x > 0. && area.y > 0.) {
		return None;
	}

	// An area too large for a texture, or for its memory, gets fewer pixels, each spanning more of the view
	let resolution = budgeted_resolution(area, view_size.element_product(), max_pixels as f64);
	let pixels = (area * resolution).round().max(DVec2::ONE);
	let document_to_pixels = DAffine2::from_scale(pixels / area) * DAffine2::from_translation(-min) * document_to_view;
	let placement = user_to_view.inverse() * DAffine2::from_translation(min) * DAffine2::from_scale(area);

	Some((placement, document_to_pixels, pixels.as_uvec2()))
}

/// The render parameters drawing into an image that `document_to_pixels` places the document in, of `size` pixels.
fn image_render_params(render_params: &RenderParams, document_to_pixels: DAffine2, size: UVec2) -> RenderParams {
	RenderParams {
		render_output_type: RenderOutputType::Vello,
		footprint: Footprint {
			transform: document_to_pixels,
			resolution: size,
			..Default::default()
		},
		..render_params.clone()
	}
}

/// The span of `svg` from the `opening` marker through the `closing` one, if both are found.
fn marked_span(svg: &str, opening: &str, closing: &str) -> Option<std::ops::Range<usize>> {
	let start = svg.find(opening)?;
	let end = svg[start..].find(closing).map(|offset| start + offset + closing.len())?;
	Some(start..end)
}

/// One map of an effect's coverage an SVG filter takes as an image the GPU renders.
#[derive(Clone)]
pub struct EffectMap {
	/// Names the markers around the placeholder in the SVG, which the map replaces.
	pub id: u64,
	/// The item with its effects, keeping its opacity and blend mode when its effects are layered, since they set how strongly the content knocks out the pieces.
	pub content: Graphic,
	pub render_params: RenderParams,
	pub capture: EffectCapture,
	/// The name of the filter result the map stands for.
	pub result: String,
	/// Maps the filter's user space into the document.
	pub user_to_document: DAffine2,
	/// Maps the user space the `content` is drawn in into the document, which differs from the filter's where the filter measures in its effects' space.
	pub content_to_document: DAffine2,
	/// The filter's region in its user space, which the map covers.
	pub bounds: [DVec2; 2],
	/// How far past the view the map covers, in document pixels, for the filters reading around it after the effect.
	pub margin: f64,
}

/// Where a filter places the map the GPU rendered of an effect's coverage.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectMapPlacement {
	/// Names the markers the map replaces.
	pub id: u64,
	pub result: String,
	/// Places the map's unit square in the filter's user space.
	pub transform: DAffine2,
}

/// The map of an effect's coverage for the GPU to render: where the filter places it, the transform mapping the document into its pixels, and how many pixels it has.
pub struct EffectMapLayout {
	pub placement: EffectMapPlacement,
	pub document_to_pixels: DAffine2,
	pub size: UVec2,
}

/// The markers that open and close the placeholder for the map `id` names.
fn map_markers(id: u64) -> (String, String) {
	(format!("<!--effect-map-{id}-->"), format!("<!--/effect-map-{id}-->"))
}

/// The filter primitive standing for the map `id` names until it's resolved, flooding the region with `flood`, which leaves the effect out.
/// Coverage that is added leaves the effect out when transparent, coverage the content is multiplied by leaves it out when opaque, and data a filter combines in other ways takes whatever value is neutral for it.
pub(crate) fn map_placeholder(id: u64, result: &str, flood: Color) -> String {
	let color = SRGBA8::from(flood.with_alpha(1.)).to_rgb_hex();
	map_placeholder_primitive(id, &format!(r##"<feFlood flood-color="#{color}" flood-opacity="{}" result="{result}" />"##, flood.a()))
}

/// The filter `primitive` standing for the map `id` names until it's resolved, which gives the result the map stands for.
pub(crate) fn map_placeholder_primitive(id: u64, primitive: &str) -> String {
	let (opening, closing) = map_markers(id);
	format!("{opening}{primitive}{closing}")
}

impl EffectMap {
	/// The map covering the part of the filter's region in a view that `document_to_view` maps the document into, spanning `view_size` pixels, with at most `max_pixels` across or down.
	/// `None` if none of it lies in view.
	pub fn layout(&self, document_to_view: DAffine2, view_size: DVec2, max_pixels: u32) -> Option<EffectMapLayout> {
		let (transform, document_to_pixels, size) = image_layout(self.user_to_document, self.bounds, self.margin, document_to_view, view_size, max_pixels)?;
		Some(EffectMapLayout {
			placement: EffectMapPlacement {
				id: self.id,
				result: self.result.clone(),
				transform,
			},
			document_to_pixels,
			size,
		})
	}

	/// Draws the effect's coverage into a scene of the document, which the `layout` places in the map's pixels.
	pub fn draw(&self, scene: &mut VelloSceneBuilder, layout: &EffectMapLayout) {
		scene.capture_next_effects(self.capture);
		self.content
			.render_to_vello(scene, self.content_to_document, &image_render_params(&self.render_params, layout.document_to_pixels, layout.size));
	}
}

/// Replaces the placeholder for a map in `svg` with the `map` the GPU rendered, as a filter image of its coverage in alpha.
pub fn substitute_effect_map(svg: &mut String, placement: &EffectMapPlacement, map: &Raster<CPU>) {
	let (opening, closing) = map_markers(placement.id);
	let Some(span) = marked_span(svg, &opening, &closing) else { return };

	let [min, max] = (placement.transform * Quad::from_box([DVec2::ZERO, DVec2::ONE])).bounding_box();
	let size = max - min;
	let png = map.png_data_url();

	// A map turned against the filter's user space goes in an SVG document of its own, which turns it into place
	let [a, b, c, d] = placement.transform.matrix2.to_cols_array();
	let href = match b == 0. && c == 0. && a > 0. && d > 0. {
		true => png.to_string(),
		false => {
			use base64::Engine;

			let [a, b, c, d, e, f] = placement.transform.to_cols_array();
			let document = format!(
				r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{} {} {} {}" width="{}" height="{}" preserveAspectRatio="none"><image width="1" height="1" preserveAspectRatio="none" transform="matrix({a} {b} {c} {d} {e} {f})" href="{png}" /></svg>"#,
				min.x, min.y, size.x, size.y, size.x, size.y
			);
			format!("data:image/svg+xml;base64,{}", base64::engine::general_purpose::STANDARD.encode(document))
		}
	};

	let image = format!(
		r#"<feImage href="{href}" x="{}" y="{}" width="{}" height="{}" preserveAspectRatio="none" result="{}" />"#,
		min.x, min.y, size.x, size.y, placement.result
	);
	svg.replace_range(span, &image);
}

/// One item of content an SVG draws as an image the GPU renders of it, since its effects hold one SVG filters can't carry out.
#[derive(Clone)]
pub struct DeferredImage {
	/// Names the markers around the content's own markup in the SVG, which the image replaces.
	pub id: u64,
	/// The item with its effects, but without its opacity and blend mode, which the image takes instead.
	pub content: Graphic,
	pub render_params: RenderParams,
	/// Maps the user space the markers sit in into the document.
	pub user_to_document: DAffine2,
	/// The content's extent with how far its effects reach past it, in the user space the markers sit in.
	pub bounds: [DVec2; 2],
	/// How far past the view the image covers, in document pixels, for the filters of groups around it reading around it.
	pub margin: f64,
	pub opacity: f32,
	pub blend_mode: BlendMode,
}

/// How an SVG draws the image the GPU rendered of an item of deferred content.
#[derive(Clone, Debug, PartialEq)]
pub struct DeferredImagePlacement {
	/// Names the markers the image replaces.
	pub id: u64,
	/// Places the image's unit square in the user space the markers sit in.
	pub transform: DAffine2,
	/// Maps the user space the markers sit in into the document.
	pub user_to_document: DAffine2,
	pub opacity: f32,
	pub blend_mode: BlendMode,
	/// Whether the image is drawn to a canvas rather than embedded in the SVG.
	pub to_canvas: bool,
	/// Screen pixels per document unit, which sizes an image drawn to a canvas.
	pub canvas_image_scale: f64,
}

/// The image of an item of deferred content for the GPU to render: where the SVG draws it, the transform mapping the document into its pixels, and how many pixels it has.
pub struct DeferredImageLayout {
	pub placement: DeferredImagePlacement,
	pub document_to_pixels: DAffine2,
	pub size: UVec2,
}

/// The markers that open and close the markup standing in for the item of deferred content `id` names.
pub(crate) fn image_markers(id: u64) -> (String, String) {
	(format!("<!--deferred-image-{id}-->"), format!("<!--/deferred-image-{id}-->"))
}

impl DeferredImage {
	/// The image covering the part of the content's reach in a view that `document_to_view` maps the document into, spanning `view_size` pixels, with at most `max_pixels` across or down.
	/// `None` if none of it lies in view.
	pub fn layout(&self, document_to_view: DAffine2, view_size: DVec2, max_pixels: u32) -> Option<DeferredImageLayout> {
		let (transform, document_to_pixels, size) = image_layout(self.user_to_document, self.bounds, self.margin, document_to_view, view_size, max_pixels)?;
		Some(DeferredImageLayout {
			placement: DeferredImagePlacement {
				id: self.id,
				transform,
				user_to_document: self.user_to_document,
				opacity: self.opacity,
				blend_mode: self.blend_mode,
				to_canvas: self.render_params.to_canvas(),
				canvas_image_scale: self.render_params.canvas_image_scale,
			},
			document_to_pixels,
			size,
		})
	}

	/// Draws the content with its effects into a scene of the document, which the `layout` places in the image's pixels.
	pub fn draw(&self, scene: &mut VelloSceneBuilder, layout: &DeferredImageLayout) {
		self.content
			.render_to_vello(scene, self.user_to_document, &image_render_params(&self.render_params, layout.document_to_pixels, layout.size));
	}
}

/// Replaces the markup standing in for an item of deferred content in `svg` with the `image` the GPU rendered of it, adding the image to `image_data` if it's drawn to a canvas.
/// Markup repeated in each layer of an ancestor's effects stands in for the content once in each.
pub fn substitute_deferred_image(svg: &mut String, image_data: &mut Vec<(u64, CanvasImage)>, placement: &DeferredImagePlacement, image: &Raster<CPU>) {
	let (opening, closing) = image_markers(placement.id);
	if marked_span(svg, &opening, &closing).is_none() {
		return;
	}

	let mut render = SvgRender::new();
	render.transform = placement.user_to_document;
	let render_params = RenderParams {
		for_export: !placement.to_canvas,
		canvas_image_scale: placement.canvas_image_scale,
		..Default::default()
	};
	render_raster_cpu_image_svg(image, placement.transform, placement.opacity, placement.blend_mode, &mut render, &render_params);

	let markup = render.svg.to_svg_string();
	while let Some(span) = marked_span(svg, &opening, &closing) {
		svg.replace_range(span, &markup);
	}
	image_data.extend(render.image_data);
}
