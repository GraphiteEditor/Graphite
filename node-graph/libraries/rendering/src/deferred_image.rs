//! Effects whose coverage SVG filters can't work out, like the shadows and glows shaped for PSD interop, which an SVG's filter takes as an image of that coverage the GPU renders: a map of it.
//! The content keeps its own markup, and only the map is an image. A placeholder stands in the filter for each map until it's resolved, leaving the effect out of an SVG that never is.

use crate::renderer::{Render, RenderOutputType, RenderParams};
use crate::vello_scene_builder::VelloSceneBuilder;
use core_types::math::quad::Quad;
use core_types::transform::Footprint;
use glam::{DAffine2, DVec2, UVec2};
use graphic_types::Graphic;
use graphic_types::effects::EffectCapture;
use graphic_types::raster_types::{CPU, Raster};

/// The most pixels across or down a map may have.
pub const MAX_DEFERRED_IMAGE_PIXELS: u32 = 8192;

/// One map of an effect's coverage an SVG filter takes as an image the GPU renders.
#[derive(Clone)]
pub struct EffectMap {
	/// Names the markers around the placeholder in the SVG, which the map replaces.
	pub id: u64,
	/// The item with its effects, but without its opacity and blend mode, which don't touch its effects' coverages.
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
fn markers(id: u64) -> (String, String) {
	(format!("<!--effect-map-{id}-->"), format!("<!--/effect-map-{id}-->"))
}

/// The filter primitive standing for the map `id` names until it's resolved, which leaves nothing covered.
pub(crate) fn placeholder(id: u64, result: &str) -> String {
	let (opening, closing) = markers(id);
	format!(r#"{opening}<feFlood flood-opacity="0" result="{result}" />{closing}"#)
}

impl EffectMap {
	/// The map covering the part of the filter's region in a view that `document_to_view` maps the document into, spanning `view_size` pixels, with at most `max_pixels` across or down.
	/// `None` if none of it lies in view.
	pub fn layout(&self, document_to_view: DAffine2, view_size: DVec2, max_pixels: u32) -> Option<EffectMapLayout> {
		let user_to_view = document_to_view * self.user_to_document;
		if !user_to_view.matrix2.determinant().is_normal() || !user_to_view.is_finite() {
			return None;
		}

		// The view's pixels the region reaches into, whole ones so the map's pixels line up with the view's
		let [min, max] = (user_to_view * Quad::from_box(self.bounds)).bounding_box();
		let (min, max) = (min.max(DVec2::ZERO).floor(), max.min(view_size).ceil());
		let area = max - min;
		if !(area.x > 0. && area.y > 0.) {
			return None;
		}

		// An area too large for a texture gets fewer pixels, each spanning more of the view
		let pixels = area.min(DVec2::splat(max_pixels as f64)).max(DVec2::ONE).round();
		let document_to_pixels = DAffine2::from_scale(pixels / area) * DAffine2::from_translation(-min) * document_to_view;

		Some(EffectMapLayout {
			placement: EffectMapPlacement {
				id: self.id,
				result: self.result.clone(),
				transform: user_to_view.inverse() * DAffine2::from_translation(min) * DAffine2::from_scale(area),
			},
			document_to_pixels,
			size: pixels.as_uvec2(),
		})
	}

	/// Draws the effect's coverage into a scene of the document, which the `layout` places in the map's pixels.
	pub fn draw(&self, scene: &mut VelloSceneBuilder, layout: &EffectMapLayout) {
		let render_params = RenderParams {
			render_output_type: RenderOutputType::Vello,
			footprint: Footprint {
				transform: layout.document_to_pixels,
				resolution: layout.size,
				..Default::default()
			},
			..self.render_params.clone()
		};

		scene.capture_next_effects(self.capture);
		self.content.render_to_vello(scene, self.content_to_document, &render_params);
	}
}

/// Replaces the placeholder for a map in `svg` with the `map` the GPU rendered, as a filter image of its coverage in alpha.
pub fn substitute_effect_map(svg: &mut String, placement: &EffectMapPlacement, map: &Raster<CPU>) {
	let (opening, closing) = markers(placement.id);
	let Some(start) = svg.find(&opening) else { return };
	let Some(end) = svg[start..].find(&closing).map(|offset| start + offset + closing.len()) else {
		return;
	};

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
	svg.replace_range(start..end, &image);
}
