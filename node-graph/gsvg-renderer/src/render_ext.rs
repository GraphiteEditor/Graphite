use crate::renderer::{RenderParams, format_transform_matrix};
use glam::{DAffine2, DVec2};
use graphene_core::consts::{LAYER_OUTLINE_STROKE_COLOR, LAYER_OUTLINE_STROKE_WEIGHT};
use graphene_core::gradient::{Gradient, GradientType};
use graphene_core::uuid::generate_uuid;
use graphene_core::vector::style::{Fill, PaintOrder, PathStyle, Stroke, StrokeAlign, StrokeCap, StrokeJoin, ViewMode};
use std::fmt::Write;

pub trait RenderExt {
	type Output;
	fn render(
		&self,
		svg_defs: &mut String,
		element_transform: DAffine2,
		stroke_transform: DAffine2,
		bounds: [DVec2; 2],
		transformed_bounds: [DVec2; 2],
		aligned_strokes: bool,
		override_paint_order: bool,
		render_params: &RenderParams,
	) -> Self::Output;
}

impl RenderExt for Gradient {
	type Output = u64;

	// /// Adds the gradient def through mutating the first argument, returning the gradient ID.
	fn render(
		&self,
		svg_defs: &mut String,
		element_transform: DAffine2,
		stroke_transform: DAffine2,
		bounds: [DVec2; 2],
		transformed_bounds: [DVec2; 2],
		_aligned_strokes: bool,
		_override_paint_order: bool,
		_render_params: &RenderParams,
	) -> Self::Output {
		// TODO: Figure out how to use `self.transform` as part of the gradient transform, since that field (`Gradient::transform`) is currently never read from, it's only written to.

		let bound_transform = DAffine2::from_scale_angle_translation(bounds[1] - bounds[0], 0., bounds[0]);
		let transformed_bound_transform = element_transform * DAffine2::from_scale_angle_translation(transformed_bounds[1] - transformed_bounds[0], 0., transformed_bounds[0]);

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

		let mod_gradient = if transformed_bound_transform.matrix2.determinant() != 0. {
			transformed_bound_transform.inverse()
		} else {
			DAffine2::IDENTITY // Ignore if the transform cannot be inverted (the bounds are zero). See issue #1944.
		};
		let mod_points = element_transform * stroke_transform * bound_transform;

		let start = mod_points.transform_point2(self.start);
		let end = mod_points.transform_point2(self.end);

		let gradient_id = generate_uuid();

		let matrix = format_transform_matrix(mod_gradient);
		let gradient_transform = if matrix.is_empty() { String::new() } else { format!(r#" gradientTransform="{}""#, matrix) };

		match self.gradient_type {
			GradientType::Linear => {
				let _ = write!(
					svg_defs,
					r#"<linearGradient id="{}" x1="{}" x2="{}" y1="{}" y2="{}"{gradient_transform}>{}</linearGradient>"#,
					gradient_id, start.x, end.x, start.y, end.y, stop
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
	fn render(
		&self,
		svg_defs: &mut String,
		element_transform: DAffine2,
		stroke_transform: DAffine2,
		bounds: [DVec2; 2],
		transformed_bounds: [DVec2; 2],
		aligned_strokes: bool,
		override_paint_order: bool,
		render_params: &RenderParams,
	) -> Self::Output {
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
				let gradient_id = gradient.render(
					svg_defs,
					element_transform,
					stroke_transform,
					bounds,
					transformed_bounds,
					aligned_strokes,
					override_paint_order,
					render_params,
				);
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
		_bounds: [DVec2; 2],
		_transformed_bounds: [DVec2; 2],
		aligned_strokes: bool,
		override_paint_order: bool,
		_render_params: &RenderParams,
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
		let paint_order = (self.paint_order != PaintOrder::StrokeAbove || override_paint_order).then_some(PaintOrder::StrokeBelow);

		// Render the needed stroke attributes
		let mut attributes = format!(r##" stroke="#{}""##, color.to_rgb_hex_srgb_from_gamma());
		if color.a() < 1. {
			let _ = write!(&mut attributes, r#" stroke-opacity="{}""#, (color.a() * 1000.).round() / 1000.);
		}
		if let Some(mut weight) = weight {
			if stroke_align.is_some() && aligned_strokes {
				weight *= 2.;
			}
			let _ = write!(&mut attributes, r#" stroke-width="{}""#, weight);
		}
		if let Some(dash_array) = dash_array {
			let _ = write!(&mut attributes, r#" stroke-dasharray="{}""#, dash_array);
		}
		if let Some(dash_offset) = dash_offset {
			let _ = write!(&mut attributes, r#" stroke-dashoffset="{}""#, dash_offset);
		}
		if let Some(stroke_cap) = stroke_cap {
			let _ = write!(&mut attributes, r#" stroke-linecap="{}""#, stroke_cap.svg_name());
		}
		if let Some(stroke_join) = stroke_join {
			let _ = write!(&mut attributes, r#" stroke-linejoin="{}""#, stroke_join.svg_name());
		}
		if let Some(stroke_join_miter_limit) = stroke_join_miter_limit {
			let _ = write!(&mut attributes, r#" stroke-miterlimit="{}""#, stroke_join_miter_limit);
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
	fn render(
		&self,
		svg_defs: &mut String,
		element_transform: DAffine2,
		stroke_transform: DAffine2,
		bounds: [DVec2; 2],
		transformed_bounds: [DVec2; 2],
		aligned_strokes: bool,
		override_paint_order: bool,
		render_params: &RenderParams,
	) -> String {
		let view_mode = render_params.view_mode;
		match view_mode {
			ViewMode::Outline => {
				let fill_attribute = Fill::None.render(
					svg_defs,
					element_transform,
					stroke_transform,
					bounds,
					transformed_bounds,
					aligned_strokes,
					override_paint_order,
					render_params,
				);
				let mut outline_stroke = Stroke::new(Some(LAYER_OUTLINE_STROKE_COLOR), LAYER_OUTLINE_STROKE_WEIGHT);
				// Outline strokes should be non-scaling by default
				outline_stroke.non_scaling = true;
				let stroke_attribute = outline_stroke.render(
					svg_defs,
					element_transform,
					stroke_transform,
					bounds,
					transformed_bounds,
					aligned_strokes,
					override_paint_order,
					render_params,
				);
				format!("{fill_attribute}{stroke_attribute}")
			}
			_ => {
				let fill_attribute = self.fill.render(
					svg_defs,
					element_transform,
					stroke_transform,
					bounds,
					transformed_bounds,
					aligned_strokes,
					override_paint_order,
					render_params,
				);
				let stroke_attribute = self
					.stroke
					.as_ref()
					.map(|stroke| {
						stroke.render(
							svg_defs,
							element_transform,
							stroke_transform,
							bounds,
							transformed_bounds,
							aligned_strokes,
							override_paint_order,
							render_params,
						)
					})
					.unwrap_or_default();
				format!("{fill_attribute}{stroke_attribute}")
			}
		}
	}
}
