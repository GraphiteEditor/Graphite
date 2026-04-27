use crate::renderer::{Render, RenderContext, RenderParams, SvgRender};
use core_types::color::Color;
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
	fn render_background_to_vello(&self, scene: &mut vello::Scene, transform: DAffine2, _context: &mut RenderContext, render_params: &RenderParams) {
		if self.contains_artboard() {
			return;
		}
		render_viewport_checkerboard_vello(scene, transform, render_params)
	}

	fn render_background_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		if self.contains_artboard() {
			return;
		}
		render_viewport_checkerboard_svg(render, render_params);
	}
}

impl<T> RenderBackground for Table<T>
where
	T: RenderBackground,
	Table<T>: Render,
{
	fn render_background_to_vello(&self, scene: &mut vello::Scene, transform: DAffine2, context: &mut RenderContext, render_params: &RenderParams) {
		if !self.contains_artboard() {
			render_viewport_checkerboard_vello(scene, transform, render_params);
			return;
		}

		for row in self.iter() {
			if !row.element.contains_artboard() {
				continue;
			}
			row.element.render_background_to_vello(scene, transform, context, render_params);
		}
	}

	fn render_background_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		if !self.contains_artboard() {
			render_viewport_checkerboard_svg(render, render_params);
			return;
		}

		for row in self.iter() {
			if !row.element.contains_artboard() {
				continue;
			}
			row.element.render_background_svg(render, render_params);
		}
	}
}

impl RenderBackground for Artboard {
	fn render_background_to_vello(&self, scene: &mut vello::Scene, transform: DAffine2, _context: &mut RenderContext, render_params: &RenderParams) {
		if render_params.hide_artboards || !render_params.to_canvas() || self.background.a() >= 1. || render_params.viewport_zoom <= 0. {
			return;
		}

		let rect = artboard_rect(self);
		checkerboard_fill_vello(scene, transform, rect, DVec2::new(rect.x0, rect.y0), render_params.viewport_zoom);
	}

	fn render_background_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		if render_params.hide_artboards || !render_params.to_canvas() || self.background.a() >= 1. || render_params.viewport_zoom <= 0. {
			return;
		}

		let rect = artboard_rect(self);
		checkerboard_fill_svg(render, rect, DVec2::new(rect.x0, rect.y0), render_params.viewport_zoom, "checkered-artboard");
	}
}

impl RenderBackground for Graphic {}
impl RenderBackground for Table<Vector> {}
impl RenderBackground for Table<Raster<CPU>> {}
impl RenderBackground for Table<Raster<GPU>> {}
impl RenderBackground for Table<Color> {}
impl RenderBackground for Table<GradientStops> {}

fn render_viewport_checkerboard_vello(scene: &mut vello::Scene, transform: DAffine2, render_params: &RenderParams) {
	if !render_params.to_canvas() {
		return;
	}
	let Some(rect) = viewport_rect(render_params.footprint, render_params.scale) else {
		return;
	};
	checkerboard_fill_vello(scene, transform, rect, DVec2::ZERO, render_params.viewport_zoom);
}

fn render_viewport_checkerboard_svg(render: &mut SvgRender, render_params: &RenderParams) {
	if !render_params.to_canvas() {
		return;
	}
	let Some(rect) = viewport_rect(render_params.footprint, render_params.scale) else {
		return;
	};
	checkerboard_fill_svg(render, rect, DVec2::ZERO, render_params.viewport_zoom, "checkered-viewport");
}

fn checkerboard_fill_vello(scene: &mut vello::Scene, transform: DAffine2, rect: kurbo::Rect, pattern_origin: DVec2, viewport_zoom: f64) {
	if viewport_zoom <= 0. {
		return;
	}

	let brush = vello::peniko::Brush::Image(vello::peniko::ImageBrush {
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
	});
	let brush_transform = kurbo::Affine::scale(1. / viewport_zoom).then_translate(kurbo::Vec2::new(pattern_origin.x, pattern_origin.y));
	scene.fill(vello::peniko::Fill::NonZero, kurbo::Affine::new(transform.to_cols_array()), &brush, Some(brush_transform), &rect);
}

fn checkerboard_fill_svg(render: &mut SvgRender, rect: kurbo::Rect, pattern_origin: DVec2, viewport_zoom: f64, checker_id_prefix: &str) {
	if viewport_zoom <= 0. {
		return;
	}

	let checker_id = format!("{checker_id_prefix}-{}", generate_uuid());

	let svg_defs: &mut String = &mut render.svg_defs;
	let pattern_id: &str = &checker_id;

	let cell_size = 8. / viewport_zoom;
	let pattern_size = cell_size * 2.;

	write!(
		svg_defs,
		r##"<pattern id="{pattern_id}" x="{}" y="{}" width="{pattern_size}" height="{pattern_size}" patternUnits="userSpaceOnUse"><rect width="{pattern_size}" height="{pattern_size}" fill="#ffffff" /><rect x="{cell_size}" y="0" width="{cell_size}" height="{cell_size}" fill="#cccccc" /><rect x="0" y="{cell_size}" width="{cell_size}" height="{cell_size}" fill="#cccccc" /></pattern>"##,
		pattern_origin.x,
		pattern_origin.y,
	)
	.unwrap();

	render.leaf_tag("rect", |attributes| {
		attributes.push("x", rect.x0.to_string());
		attributes.push("y", rect.y0.to_string());
		attributes.push("width", rect.width().to_string());
		attributes.push("height", rect.height().to_string());
		attributes.push("fill", format!("url(#{checker_id})"));
	});
}

fn artboard_rect(artboard: &Artboard) -> kurbo::Rect {
	let [a, b] = [artboard.location.as_dvec2(), artboard.location.as_dvec2() + artboard.dimensions.as_dvec2()];
	kurbo::Rect::new(a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y))
}

fn viewport_rect(footprint: Footprint, scale: f64) -> Option<kurbo::Rect> {
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
