use crate::renderer::{RenderParams, format_transform_matrix, gradient_placement, transform_is_invertible};
use crate::{Render, RenderSvgSegmentList, SvgRender};
use core_types::color::SRGBA8;
use core_types::list::List;
use core_types::uuid::generate_uuid;
use core_types::{Color, attr};
use glam::{DAffine2, DVec2};
use graphic_types::Graphic;
use graphic_types::vector_types::gradient::GradientType;
use graphic_types::vector_types::vector::style::{PaintOrder, Stroke, StrokeAlign, StrokeCap, StrokeJoin};
use std::fmt::Write;
use vector_types::Gradient;
use vector_types::gradient::GradientSpreadMethod;

#[derive(Copy, Clone, PartialEq)]
pub enum PaintTarget {
	Fill,
	Stroke,
}

impl PaintTarget {
	fn paint_attr(self) -> &'static str {
		match self {
			Self::Fill => "fill",
			Self::Stroke => "stroke",
		}
	}

	fn opacity_attr(self) -> &'static str {
		match self {
			Self::Fill => "fill-opacity",
			Self::Stroke => "stroke-opacity",
		}
	}
}

pub trait RenderExt {
	type Output;

	#[allow(clippy::too_many_arguments)]
	fn render(
		&self,
		svg_defs: &mut String,
		item_transform: DAffine2,
		element_transform: DAffine2,
		stroke_transform: DAffine2,
		bounds: DAffine2,
		render_params: &RenderParams,
		target: PaintTarget,
	) -> Self::Output;
}

impl RenderExt for List<Color> {
	type Output = String;

	fn render(
		&self,
		_svg_defs: &mut String,
		_item_transform: DAffine2,
		_element_transform: DAffine2,
		_stroke_transform: DAffine2,
		_bounds: DAffine2,
		_render_params: &RenderParams,
		target: PaintTarget,
	) -> Self::Output {
		let Some(color) = self.element(0) else {
			return format!(r#" {}="none""#, target.paint_attr());
		};

		let mut result = format!(r##" {}="#{}""##, target.paint_attr(), SRGBA8::from(*color).to_rgb_hex());
		if color.a() < 1. {
			let _ = write!(result, r#" {}="{}""#, target.opacity_attr(), (color.a() * 1000.).round() / 1000.);
		}

		result
	}
}

impl RenderExt for List<Gradient> {
	type Output = u64;

	/// Adds the gradient def through mutating the first argument, returning the gradient ID.
	fn render(
		&self,
		svg_defs: &mut String,
		item_transform: DAffine2,
		element_transform: DAffine2,
		_stroke_transform: DAffine2,
		_bounds: DAffine2,
		_render_params: &RenderParams,
		_target: PaintTarget,
	) -> Self::Output {
		let mut stop = String::new();

		let Some(stops) = self.element(0) else { return 0 };
		let gradient_type = self.attr_cloned_or_default::<vector_types::attr::GradientType>(0);
		let local_gradient_transform = self.attr_cloned_or_default::<attr::Transform>(0);
		let spread_method = self.attr_cloned_or_default::<vector_types::attr::SpreadMethod>(0);

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

		// Need to cancel out the element's transform as it is already applied to the path itself.
		let element_transform_inverse = if transform_is_invertible(element_transform) {
			element_transform.inverse()
		} else {
			DAffine2::IDENTITY
		};

		let document_transform = item_transform * local_gradient_transform;

		let placement = gradient_placement(document_transform, gradient_type);
		let gradient_transform = format_transform_matrix(element_transform_inverse * placement);
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
					r#"<linearGradient id="{}" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="1" y2="0"{spread_method}{gradient_transform}>{}</linearGradient>"#,
					gradient_id, stop
				);
			}
			GradientType::Radial => {
				let _ = write!(
					svg_defs,
					r#"<radialGradient id="{}" gradientUnits="userSpaceOnUse" cx="0" cy="0" r="1"{spread_method}{gradient_transform}>{}</radialGradient>"#,
					gradient_id, stop
				);
			}
		}

		gradient_id
	}
}

impl RenderExt for Stroke {
	type Output = String;

	/// Provide the shape-related SVG attributes for the stroke. The paint-related attributes for the stroke are generated from `List<Graphic>.render` with `PaintTarget::Stroke`.
	fn render(
		&self,
		_svg_defs: &mut String,
		_item_transform: DAffine2,
		_element_transform: DAffine2,
		_stroke_transform: DAffine2,
		_bounds: DAffine2,
		render_params: &RenderParams,
		_target: PaintTarget,
	) -> Self::Output {
		// Don't render a stroke at all if it would be invisible
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
		let mut attributes = String::new();
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

impl RenderExt for List<Graphic> {
	type Output = String;

	fn render(
		&self,
		svg_defs: &mut String,
		item_transform: DAffine2,
		element_transform: DAffine2,
		stroke_transform: DAffine2,
		bounds: DAffine2,
		render_params: &RenderParams,
		target: PaintTarget,
	) -> Self::Output {
		let fill_graphic = self.element(0);
		let paint_attr = target.paint_attr();

		match fill_graphic {
			Some(Graphic::Color(color_list)) => color_list.render(svg_defs, item_transform, element_transform, stroke_transform, bounds, render_params, target),
			Some(Graphic::Gradient(gradient_list)) => {
				let gradient_id = gradient_list.render(svg_defs, item_transform, element_transform, stroke_transform, bounds, render_params, target);
				format!(r##" {paint_attr}="url(#{gradient_id})""##)
			}
			Some(Graphic::None) => format!(r#" {paint_attr}="none""#),
			Some(Graphic::Vector(_)) | Some(Graphic::RasterCPU(_)) | Some(Graphic::RasterGPU(_)) | Some(Graphic::Graphic(_)) | Some(Graphic::Text(_)) => {
				let bounds = if target == PaintTarget::Stroke {
					// To prevent a wraparound artefact occurring when the tile boundary and the stroke region are perfectly aligned, the local coordinate is expanded slightly.
					let inverse = |len: f64| if len > 0. { 1. / len } else { 0. };
					let inflate = DVec2::new(inverse(item_transform.matrix2.x_axis.length()), inverse(item_transform.matrix2.y_axis.length()));
					let min = bounds.transform_point2(DVec2::ZERO) - inflate;
					let max = bounds.transform_point2(DVec2::ONE) + inflate;
					DAffine2::from_scale_angle_translation(max - min, 0., min)
				} else {
					bounds
				};
				render_svg_pattern(svg_defs, self, stroke_transform, bounds, render_params)
					.map(|id| format!(r##" {paint_attr}="url(#{id})""##))
					.unwrap_or_else(|| format!(r#" {paint_attr}="none""#))
			}
			None => format!(r#" {paint_attr}="none""#),
		}
	}
}

/// Emits an SVG `<pattern>` paint server into `svg_defs` that renders the given graphic list as the paint content, and returns the pattern ID.
/// Currently, this function is only used for clipping-based filling and stroking, not considering tiling yet.
fn render_svg_pattern(svg_defs: &mut String, fill_graphic_list: &List<Graphic>, stroke_transform: DAffine2, bounds: DAffine2, render_params: &RenderParams) -> Option<String> {
	let min = bounds.transform_point2(DVec2::ZERO);
	let max = bounds.transform_point2(DVec2::ONE);
	let size = max - min;
	if size.x <= 0. || size.y <= 0. {
		return None;
	}

	// Render the pattern content recursively
	let mut content = SvgRender::new();
	fill_graphic_list.render_svg(&mut content, &render_params.for_pattern());

	// Unwrap the inner def element
	write!(svg_defs, "{}", content.svg_defs).unwrap();

	let pattern_transform = stroke_transform * DAffine2::from_translation(min);
	let transform_str = format_transform_matrix(pattern_transform);
	let transform_attr = if transform_str.is_empty() {
		String::new()
	} else {
		format!(r#" patternTransform="{transform_str}""#)
	};

	let pattern_id = format!("pattern-{}", generate_uuid());
	write!(
		svg_defs,
		r##"<pattern id="{pattern_id}" patternUnits="userSpaceOnUse" x="0" y="0" width="{}" height="{}"{transform_attr}>"##,
		size.x, size.y,
	)
	.unwrap();

	let content_shift = format_transform_matrix(DAffine2::from_translation(-min));
	write!(svg_defs, r##"<g transform="{content_shift}">{}</g></pattern>"##, content.svg.to_svg_string()).unwrap();

	Some(pattern_id)
}
