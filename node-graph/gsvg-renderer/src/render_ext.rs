use crate::renderer::{RenderParams, format_transform_matrix};
use glam::DAffine2;
use graphene_core::consts::{LAYER_OUTLINE_STROKE_COLOR, LAYER_OUTLINE_STROKE_WEIGHT};
use graphene_core::gradient::{Gradient, GradientType};
use graphene_core::uuid::generate_uuid;
use graphene_core::vector::style::{Fill, PaintOrder, PathStyle, RenderMode, Stroke, StrokeAlign, StrokeCap, StrokeJoin};
use std::fmt::Write;

pub trait RenderExt {
	type Output;
	fn render(&self, svg_defs: &mut String, element_transform: DAffine2, stroke_transform: DAffine2, bounds: DAffine2, transformed_bounds: DAffine2, render_params: &RenderParams) -> Self::Output;
}

impl RenderExt for Gradient {
	type Output = u64;

	// /// Adds the gradient def through mutating the first argument, returning the gradient ID.
	fn render(&self, svg_defs: &mut String, element_transform: DAffine2, stroke_transform: DAffine2, bounds: DAffine2, transformed_bounds: DAffine2, _render_params: &RenderParams) -> Self::Output {
		let mut stop = String::new();
		for (position, color) in self.stops.0.iter() {
			stop.push_str("<stop");
			if *position != 0. {
				let _ = write!(stop, r#" offset="{}""#, (position * 1_000_000.).round() / 1_000_000.);
			}
			let _ = write!(stop, r##" stop-color="#{}""##, color.to_rgb_hex_srgb_from_gamma());
			if color.a() < 1. {
				let _ = write!(stop, r#" stop-opacity="{}""#, (color.a() * 1000.).round() / 1000.);
			}
			stop.push_str(" />")
		}

		let transform_points = element_transform * stroke_transform * bounds;
		let start = transform_points.transform_point2(self.start);
		let end = transform_points.transform_point2(self.end);

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

		let gradient_id = generate_uuid();

		match self.gradient_type {
			GradientType::Linear => {
				let _ = write!(
					svg_defs,
					r#"<linearGradient id="{}" x1="{}" y1="{}" x2="{}" y2="{}"{gradient_transform}>{}</linearGradient>"#,
					gradient_id, start.x, start.y, end.x, end.y, stop
				);
			}
			GradientType::Radial => {
				let radius = (f64::powi(start.x - end.x, 2) + f64::powi(start.y - end.y, 2)).sqrt();
				let _ = write!(
					svg_defs,
					r#"<radialGradient id="{}" cx="{}" cy="{}" r="{}"{gradient_transform}>{}</radialGradient>"#,
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
		match self {
			Self::None => r#" fill="none""#.to_string(),
			Self::Solid(color) => {
				let mut result = format!(r##" fill="#{}""##, color.to_rgb_hex_srgb_from_gamma());
				if color.a() < 1. {
					let _ = write!(result, r#" fill-opacity="{}""#, (color.a() * 1000.).round() / 1000.);
				}
				result
			}
			Self::Gradient(gradient) => {
				let gradient_id = gradient.render(svg_defs, element_transform, stroke_transform, bounds, transformed_bounds, render_params);
				format!(r##" fill="url('#{gradient_id}')""##)
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

		// Set to None if the value is the SVG default
		let weight = (self.weight != 1.).then_some(self.weight);
		let dash_array = (!self.dash_lengths.is_empty()).then_some(self.dash_lengths());
		let dash_offset = (self.dash_offset != 0.).then_some(self.dash_offset);
		let stroke_cap = (self.cap != StrokeCap::Butt).then_some(self.cap);
		let stroke_join = (self.join != StrokeJoin::Miter).then_some(self.join);
		let stroke_join_miter_limit = (self.join_miter_limit != 4.).then_some(self.join_miter_limit);
		let stroke_align = (self.align != StrokeAlign::Center).then_some(self.align);
		let paint_order = (self.paint_order != PaintOrder::StrokeAbove || render_params.override_paint_order).then_some(PaintOrder::StrokeBelow);

		// Render the needed stroke attributes
		let mut attributes = format!(r##" stroke="#{}""##, color.to_rgb_hex_srgb_from_gamma());
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
		// Add vector-effect attribute to make strokes non-scaling
		if self.non_scaling {
			let _ = write!(&mut attributes, r#" vector-effect="non-scaling-stroke""#);
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
		let render_mode = render_params.render_mode;
		match render_mode {
			RenderMode::Outline => {
				let fill_attribute = Fill::None.render(svg_defs, element_transform, stroke_transform, bounds, transformed_bounds, render_params);
				let mut outline_stroke = Stroke::new(Some(LAYER_OUTLINE_STROKE_COLOR), LAYER_OUTLINE_STROKE_WEIGHT);
				// Outline strokes should be non-scaling by default
				outline_stroke.non_scaling = true;
				let stroke_attribute = outline_stroke.render(svg_defs, element_transform, stroke_transform, bounds, transformed_bounds, render_params);
				format!("{fill_attribute}{stroke_attribute}")
			}
			_ => {
				let fill_attribute = self.fill.render(svg_defs, element_transform, stroke_transform, bounds, transformed_bounds, render_params);
				let stroke_attribute = self
					.stroke
					.as_ref()
					.map(|stroke| stroke.render(svg_defs, element_transform, stroke_transform, bounds, transformed_bounds, render_params))
					.unwrap_or_default();
				format!("{fill_attribute}{stroke_attribute}")
			}
		}
	}
}
