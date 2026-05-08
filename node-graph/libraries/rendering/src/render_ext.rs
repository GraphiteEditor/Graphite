use crate::renderer::{RenderParams, format_transform_matrix};
use core_types::table::Table;
use core_types::color::SRGBA8;
use core_types::uuid::generate_uuid;
use core_types::{ATTR_GRADIENT_TYPE, ATTR_SPREAD_METHOD, ATTR_TRANSFORM, Color};
use glam::{DAffine2, DVec2};
use graphic_types::Graphic;
use graphic_types::graphic::fill_to_paint;
use graphic_types::vector_types::gradient::GradientType;
use graphic_types::vector_types::vector::style::{Fill, PaintOrder, PathStyle, Stroke, StrokeAlign, StrokeCap, StrokeJoin};
use std::fmt::Write;
use vector_types::GradientStops;
use vector_types::gradient::GradientSpreadMethod;

pub trait RenderExt {
	type Output;
	fn render(&self, svg_defs: &mut String, element_transform: DAffine2, stroke_transform: DAffine2, bounds: DAffine2, transformed_bounds: DAffine2, render_params: &RenderParams) -> Self::Output;
}

impl RenderExt for Table<Color> {
	type Output = String;

	fn render(
		&self,
		_svg_defs: &mut String,
		_element_transform: DAffine2,
		_stroke_transform: DAffine2,
		_bounds: DAffine2,
		_transformed_bounds: DAffine2,
		_render_params: &RenderParams,
	) -> Self::Output {
		let Some(color) = self.element(0) else { return String::new() };

		let mut result = format!(r##" fill="#{}""##, SRGBA8::from(*color).to_rgb_hex());
		if color.a() < 1. {
			let _ = write!(result, r#" fill-opacity="{}""#, (color.a() * 1000.).round() / 1000.);
		}

		result
	}
}

impl RenderExt for Table<GradientStops> {
	type Output = u64;

	/// Adds the gradient def through mutating the first argument, returning the gradient ID.
	fn render(&self, svg_defs: &mut String, element_transform: DAffine2, stroke_transform: DAffine2, bounds: DAffine2, transformed_bounds: DAffine2, _render_params: &RenderParams) -> Self::Output {
		let mut stop = String::new();

		let Some(stops) = self.element(0) else { return 0 };
		let gradient_type: GradientType = self.attribute_cloned_or_default(ATTR_GRADIENT_TYPE, 0);
		let gradient_transform: DAffine2 = self.attribute_cloned_or_default(ATTR_TRANSFORM, 0);
		let spread_method: GradientSpreadMethod = self.attribute_cloned_or_default(ATTR_SPREAD_METHOD, 0);

		for (position, color, original_midpoint) in stops.interpolated_samples() {
			stop.push_str("<stop");
			if position != 0. {
				let _ = write!(stop, r#" offset="{}""#, (position * 1_000_000.).round() / 1_000_000.);
			}
			let _ = write!(stop, r##" stop-color="#{}""##, SRGBA8::from(color).to_rgb_hex());
			if color.a() < 1. {
				let _ = write!(stop, r#" stop-opacity="{}""#, (color.a() * 1000.).round() / 1000.);
			}
			if let Some(midpoint) = original_midpoint {
				let _ = write!(stop, r#" graphite:midpoint="{}""#, (midpoint * 1000.).round() / 1000.);
			}
			stop.push_str(" />")
		}

		let transform_points = element_transform * stroke_transform * bounds * gradient_transform;
		let start = transform_points.transform_point2(DVec2::ZERO);
		let end = transform_points.transform_point2(DVec2::X);

		let gradient_transform = if transformed_bounds.matrix2.determinant() != 0. {
			transformed_bounds.inverse()
		} else {
			DAffine2::IDENTITY // Ignore if the transform cannot be inverted (the bounds are zero). See issue #1944.
		};
		let gradient_transform = format_transform_matrix(gradient_transform);
		let gradient_transform = if gradient_transform.is_empty() {
			String::new()
		} else {
			format!(r#" gradientTransform="{gradient_transform}""#)
		};

		let spread_method = if spread_method == GradientSpreadMethod::Pad {
			String::new()
		} else {
			format!(r#" spreadMethod="{}""#, spread_method.svg_name())
		};

		let gradient_id = generate_uuid();

		match gradient_type {
			GradientType::Linear => {
				let _ = write!(
					svg_defs,
					r#"<linearGradient id="{}" x1="{}" y1="{}" x2="{}" y2="{}"{spread_method}{gradient_transform}>{}</linearGradient>"#,
					gradient_id, start.x, start.y, end.x, end.y, stop
				);
			}
			GradientType::Radial => {
				let radius = (f64::powi(start.x - end.x, 2) + f64::powi(start.y - end.y, 2)).sqrt();
				let _ = write!(
					svg_defs,
					r#"<radialGradient id="{}" cx="{}" cy="{}" r="{}"{spread_method}{gradient_transform}>{}</radialGradient>"#,
					gradient_id, start.x, start.y, radius, stop
				);
			}
		}

		gradient_id
	}
}

impl RenderExt for Fill {
	type Output = String;

	/// Renders the fill, adding necessary defs through mutating the first argument.
	fn render(&self, svg_defs: &mut String, element_transform: DAffine2, stroke_transform: DAffine2, bounds: DAffine2, transformed_bounds: DAffine2, render_params: &RenderParams) -> Self::Output {
		let Some(paint_table) = fill_to_paint(self) else { return r#" fill="none""#.to_string() };
		let Some(paint) = paint_table.element(0) else { return String::new() };

		match paint {
			Graphic::Color(color_table) => color_table.render(svg_defs, element_transform, stroke_transform, bounds, transformed_bounds, render_params),
			Graphic::Gradient(stops_table) => {
				let gradient_id = stops_table.render(svg_defs, element_transform, stroke_transform, bounds, transformed_bounds, render_params);
				format!(r##" fill="url('#{gradient_id}')""##)
			}
			_ => {
				todo!()
			}
		}
	}
}

impl RenderExt for Stroke {
	type Output = String;

	/// Provide the SVG attributes for the stroke.
	fn render(
		&self,
		_svg_defs: &mut String,
		_element_transform: DAffine2,
		_stroke_transform: DAffine2,
		_bounds: DAffine2,
		_transformed_bounds: DAffine2,
		render_params: &RenderParams,
	) -> Self::Output {
		// Don't render a stroke at all if it would be invisible
		let Some(color) = self.color else { return String::new() };
		if !self.has_renderable_stroke() {
			return String::new();
		}

		let default_weight = if self.align != StrokeAlign::Center && render_params.aligned_strokes { 1. / 2. } else { 1. };

		// Set to None if the value is the SVG default
		let weight = (self.weight != default_weight).then_some(self.weight);
		let dash_array = (!self.dash_lengths.is_empty()).then_some(self.dash_lengths());
		let dash_offset = (self.dash_offset != 0.).then_some(self.dash_offset);
		let stroke_cap = (self.cap != StrokeCap::Butt).then_some(self.cap);
		let stroke_join = (self.join != StrokeJoin::Miter).then_some(self.join);
		let stroke_join_miter_limit = (self.join_miter_limit != 4.).then_some(self.join_miter_limit);
		let stroke_align = (self.align != StrokeAlign::Center).then_some(self.align);
		let paint_order = (self.paint_order != PaintOrder::StrokeAbove || render_params.override_paint_order).then_some(PaintOrder::StrokeBelow);

		// Render the needed stroke attributes
		let mut attributes = format!(r##" stroke="#{}""##, SRGBA8::from(color).to_rgb_hex());
		if color.a() < 1. {
			let _ = write!(&mut attributes, r#" stroke-opacity="{}""#, (color.a() * 1000.).round() / 1000.);
		}
		if let Some(mut weight) = weight {
			if stroke_align.is_some() && render_params.aligned_strokes {
				weight *= 2.;
			}
			let _ = write!(&mut attributes, r#" stroke-width="{weight}""#);
		}
		if let Some(dash_array) = dash_array {
			let _ = write!(&mut attributes, r#" stroke-dasharray="{dash_array}""#);
		}
		if let Some(dash_offset) = dash_offset {
			let _ = write!(&mut attributes, r#" stroke-dashoffset="{dash_offset}""#);
		}
		if let Some(stroke_cap) = stroke_cap {
			let _ = write!(&mut attributes, r#" stroke-linecap="{}""#, stroke_cap.svg_name());
		}
		if let Some(stroke_join) = stroke_join {
			let _ = write!(&mut attributes, r#" stroke-linejoin="{}""#, stroke_join.svg_name());
		}
		if let Some(stroke_join_miter_limit) = stroke_join_miter_limit {
			let _ = write!(&mut attributes, r#" stroke-miterlimit="{stroke_join_miter_limit}""#);
		}
		if paint_order.is_some() {
			let _ = write!(&mut attributes, r#" style="paint-order: stroke;" "#);
		}
		attributes
	}
}

impl RenderExt for PathStyle {
	type Output = String;

	/// Renders the shape's fill and stroke attributes as a string with them concatenated together.
	#[allow(clippy::too_many_arguments)]
	fn render(&self, svg_defs: &mut String, element_transform: DAffine2, stroke_transform: DAffine2, bounds: DAffine2, transformed_bounds: DAffine2, render_params: &RenderParams) -> String {
		let fill_attribute = self.fill.render(svg_defs, element_transform, stroke_transform, bounds, transformed_bounds, render_params);
		let stroke_attribute = self
			.stroke
			.as_ref()
			.map(|stroke| stroke.render(svg_defs, element_transform, stroke_transform, bounds, transformed_bounds, render_params))
			.unwrap_or_default();
		format!("{fill_attribute}{stroke_attribute}")
	}
}
