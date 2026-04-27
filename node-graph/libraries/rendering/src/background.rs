use crate::renderer::{Render, RenderContext, RenderParams, SvgRender};
use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::color::Color;
use core_types::render_complexity::RenderComplexity;
use core_types::table::Table;
use core_types::transform::Footprint;
use core_types::uuid::generate_uuid;
use glam::DAffine2;
use glam::DVec2;
use graphic_types::raster_types::{CPU, GPU, Raster};
use graphic_types::vector_types::gradient::GradientStops;
use graphic_types::{Artboard, Graphic, Vector};
use std::fmt::Write;
use std::sync::{Arc, LazyLock};

pub trait RenderBackground: Render {
	fn render_background_svg(&self, _render: &mut SvgRender, _render_params: &RenderParams) {}

	fn render_background_to_vello(&self, _scene: &mut vello::Scene, _transform: DAffine2, _context: &mut RenderContext, _render_params: &RenderParams) {}
}

impl RenderBackground for Artboard {
	fn render_background_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		if render_params.hide_artboards || !render_params.to_canvas() || self.background.a() >= 1. || render_params.viewport_zoom <= 0. {
			return;
		}

		let x = self.location.x.min(self.location.x + self.dimensions.x);
		let y = self.location.y.min(self.location.y + self.dimensions.y);
		let width = self.dimensions.x.abs();
		let height = self.dimensions.y.abs();
		let checker_id = format!("checkered-artboard-{}", generate_uuid());
		if !write_checkerboard_pattern(&mut render.svg_defs, &checker_id, DVec2::new(x as f64, y as f64), render_params.viewport_zoom) {
			return;
		}

		render.leaf_tag("rect", |attributes| {
			attributes.push("x", x.to_string());
			attributes.push("y", y.to_string());
			attributes.push("width", width.to_string());
			attributes.push("height", height.to_string());
			attributes.push("fill", format!("url(#{checker_id})"));
		});
	}

	fn render_background_to_vello(&self, scene: &mut vello::Scene, transform: DAffine2, _context: &mut RenderContext, render_params: &RenderParams) {
		if render_params.hide_artboards || !render_params.to_canvas() || self.background.a() >= 1. || render_params.viewport_zoom <= 0. {
			return;
		}

		let [a, b] = [self.location.as_dvec2(), self.location.as_dvec2() + self.dimensions.as_dvec2()];
		let rect = kurbo::Rect::new(a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y));
		let artboard_transform = kurbo::Affine::new(transform.to_cols_array());
		let Some(brush_transform) = checkerboard_brush_transform(render_params.viewport_zoom, DVec2::new(rect.x0, rect.y0)) else {
			return;
		};

		scene.fill(vello::peniko::Fill::NonZero, artboard_transform, &checkerboard_brush(), Some(brush_transform), &rect);
	}
}

impl RenderBackground for Table<Artboard> {
	fn render_background_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		for artboard in self.iter() {
			artboard.element.render_background_svg(render, render_params);
		}
	}

	fn render_background_to_vello(&self, scene: &mut vello::Scene, transform: DAffine2, context: &mut RenderContext, render_params: &RenderParams) {
		for row in self.iter() {
			row.element.render_background_to_vello(scene, transform * *row.transform, context, render_params);
		}
	}
}

impl RenderBackground for Graphic {}
impl RenderBackground for Table<Graphic> {}
impl RenderBackground for Table<Vector> {}
impl RenderBackground for Table<Raster<CPU>> {}
impl RenderBackground for Table<Raster<GPU>> {}
impl RenderBackground for Table<Color> {}
impl RenderBackground for Table<GradientStops> {}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Background;

impl BoundingBox for Background {
	fn bounding_box(&self, _transform: DAffine2, _include_stroke: bool) -> RenderBoundingBox {
		RenderBoundingBox::Infinite
	}
}

impl RenderComplexity for Background {}

impl Render for Background {
	fn render_svg(&self, _render: &mut SvgRender, _render_params: &RenderParams) {}

	fn render_to_vello(&self, _scene: &mut vello::Scene, _transform: DAffine2, _context: &mut RenderContext, _render_params: &RenderParams) {}
}

impl RenderBackground for Background {
	fn render_background_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		if !render_params.to_canvas() {
			return;
		}

		let Some(rect) = viewport_checkerboard_rect(render_params.footprint, render_params.scale) else {
			return;
		};

		let checker_id = format!("checkered-viewport-{}", generate_uuid());
		if write_checkerboard_pattern(&mut render.svg_defs, &checker_id, DVec2::ZERO, render_params.viewport_zoom) {
			render.leaf_tag("rect", |attributes| {
				attributes.push("x", rect.x0.to_string());
				attributes.push("y", rect.y0.to_string());
				attributes.push("width", rect.width().to_string());
				attributes.push("height", rect.height().to_string());
				attributes.push("fill", format!("url(#{checker_id})"));
			});
		}
	}

	fn render_background_to_vello(&self, scene: &mut vello::Scene, transform: DAffine2, _context: &mut RenderContext, render_params: &RenderParams) {
		if !render_params.to_canvas() {
			return;
		}

		let Some(rect) = viewport_checkerboard_rect(render_params.footprint, render_params.scale) else {
			return;
		};
		let Some(brush_transform) = checkerboard_brush_transform(render_params.viewport_zoom, DVec2::ZERO) else {
			return;
		};

		scene.fill(
			vello::peniko::Fill::NonZero,
			kurbo::Affine::new(transform.to_cols_array()),
			&checkerboard_brush(),
			Some(brush_transform),
			&rect,
		);
	}
}

/// Cached 16x16 transparency checkerboard image data (four 8x8 cells of #ffffff and #cccccc).
static CHECKERBOARD_IMAGE_DATA: LazyLock<Arc<Vec<u8>>> = LazyLock::new(|| {
	const SIZE: u32 = 16;
	const HALF: u32 = 8;

	let mut data = vec![0_u8; (SIZE * SIZE * 4) as usize];
	for y in 0..SIZE {
		for x in 0..SIZE {
			let is_light = ((x / HALF) + (y / HALF)).is_multiple_of(2);
			let value = if is_light { 0xff } else { 0xcc };
			let index = ((y * SIZE + x) * 4) as usize;
			data[index] = value;
			data[index + 1] = value;
			data[index + 2] = value;
			data[index + 3] = 0xff;
		}
	}

	Arc::new(data)
});

fn checkerboard_brush() -> vello::peniko::Brush {
	vello::peniko::Brush::Image(vello::peniko::ImageBrush {
		image: vello::peniko::ImageData {
			data: vello::peniko::Blob::new(CHECKERBOARD_IMAGE_DATA.clone()),
			format: vello::peniko::ImageFormat::Rgba8,
			width: 16,
			height: 16,
			alpha_type: vello::peniko::ImageAlphaType::Alpha,
		},
		sampler: vello::peniko::ImageSampler {
			x_extend: vello::peniko::Extend::Repeat,
			y_extend: vello::peniko::Extend::Repeat,
			quality: vello::peniko::ImageQuality::Low,
			alpha: 1.,
		},
	})
}

fn checkerboard_brush_transform(viewport_zoom: f64, pattern_origin: DVec2) -> Option<kurbo::Affine> {
	if viewport_zoom <= 0. {
		return None;
	}

	Some(kurbo::Affine::scale(1. / viewport_zoom).then_translate(kurbo::Vec2::new(pattern_origin.x, pattern_origin.y)))
}

fn write_checkerboard_pattern(svg_defs: &mut String, pattern_id: &str, pattern_origin: DVec2, viewport_zoom: f64) -> bool {
	if viewport_zoom <= 0. {
		return false;
	}

	let cell_size = 8. / viewport_zoom;
	let pattern_size = cell_size * 2.;

	write!(
		svg_defs,
		r##"<pattern id="{pattern_id}" x="{}" y="{}" width="{pattern_size}" height="{pattern_size}" patternUnits="userSpaceOnUse"><rect width="{pattern_size}" height="{pattern_size}" fill="#ffffff" /><rect x="{cell_size}" y="0" width="{cell_size}" height="{cell_size}" fill="#cccccc" /><rect x="0" y="{cell_size}" width="{cell_size}" height="{cell_size}" fill="#cccccc" /></pattern>"##,
		pattern_origin.x,
		pattern_origin.y,
	)
	.unwrap();

	true
}

fn viewport_checkerboard_rect(footprint: Footprint, scale: f64) -> Option<kurbo::Rect> {
	if scale <= 0. {
		return None;
	}

	let logical_resolution = footprint.resolution.as_dvec2() / scale;
	let logical_footprint = Footprint {
		resolution: logical_resolution.round().as_uvec2().max(glam::UVec2::ONE),
		..footprint
	};
	let bounds = logical_footprint.viewport_bounds_in_local_space();
	let min = bounds.start.floor();
	let max = bounds.end.ceil();

	if !(min.is_finite() && max.is_finite()) {
		return None;
	}

	Some(kurbo::Rect::new(min.x, min.y, max.x, max.y))
}
