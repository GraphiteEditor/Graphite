use crate::renderer::{ClearGuardPlacement, ItemRef, RenderParams, format_transform_matrix, gradient_placement, gradient_settings_from_item, spread_adjusted_samples, transform_is_invertible};
use crate::{Render, RenderSvgSegmentList, SvgRender};
use core_types::color::SRGBA8;
use core_types::list::List;
use core_types::uuid::generate_uuid;
use core_types::{ATTR_GRADIENT_FORM, ATTR_TRANSFORM, Color};
use glam::{DAffine2, DVec2};
use graphic_types::Graphic;
use graphic_types::vector_types::gradient::GradientForm;
use graphic_types::vector_types::vector::style::{Stroke, StrokeAlign, StrokeCap, StrokeJoin};
use std::fmt::Write;
use vector_types::Gradient;
use vector_types::gradient::GradientSpread;

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

/// The paint attribute for a solid color, or the SVG `none` keyword when the color is absent.
/// `for_mask` keeps the fill opacity at full, as [`ItemRef::paint_opacity`] explains.
fn render_color_paint(item: Option<ItemRef<'_, Color>>, target: PaintTarget, for_mask: bool) -> String {
	let unpainted = || format!(r#" {}="none""#, target.paint_attr());

	let Some(item) = item else { return unpainted() };
	let Some(color) = item.element() else { return unpainted() };

	let alpha = color.a() * item.paint_opacity(for_mask);

	let mut result = format!(r##" {}="#{}""##, target.paint_attr(), SRGBA8::from(*color).to_rgb_hex());
	if alpha < 1. {
		let _ = write!(result, r#" {}="{}""#, target.opacity_attr(), (alpha * 1000.).round() / 1000.);
	}

	result
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
		render_params: &RenderParams,
		target: PaintTarget,
	) -> Self::Output {
		render_color_paint((!self.is_empty()).then_some(ItemRef::ListItem(self, 0)), target, render_params.for_mask)
	}
}

/// Adds one gradient item's def into `svg_defs` and returns the gradient ID, or `None` when the item is absent.
/// `for_mask` keeps the fill opacity at full, as [`ItemRef::paint_opacity`] explains.
fn render_gradient_paint(item: Option<ItemRef<'_, Gradient>>, svg_defs: &mut String, item_transform: DAffine2, element_transform: DAffine2, for_mask: bool) -> Option<u64> {
	let mut stop = String::new();

	let item = item?;
	let stops = item.element()?;
	let gradient_form: GradientForm = item.attribute_cloned_or_default(ATTR_GRADIENT_FORM);
	let local_gradient_transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
	let settings = gradient_settings_from_item(item);

	let (mut samples, _) = spread_adjusted_samples(stops, settings, gradient_form, ClearGuardPlacement::SvgStopOrder);

	let paint_opacity = item.paint_opacity(for_mask);
	if paint_opacity < 1. {
		for (_, color, _) in &mut samples {
			*color = color.with_alpha(color.a() * paint_opacity);
		}
	}

	for (position, color, original_midpoint) in samples {
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

	// A gradient with no stops paints as solid black, matching `Gradient::evaluate` (a stopless def would otherwise render as no paint per the SVG spec)
	if stop.is_empty() {
		stop.push_str(r##"<stop stop-color="#000000""##);
		if paint_opacity < 1. {
			let _ = write!(stop, r#" stop-opacity="{}""#, (paint_opacity * 1000.).round() / 1000.);
		}
		stop.push_str(" />");
	}

	// Need to cancel out the element's transform as it is already applied to the path itself.
	let element_transform_inverse = if transform_is_invertible(element_transform) {
		element_transform.inverse()
	} else {
		DAffine2::IDENTITY
	};

	let document_transform = item_transform * local_gradient_transform;

	let placement = gradient_placement(document_transform, gradient_form);
	let gradient_transform = format_transform_matrix(element_transform_inverse * placement);
	let gradient_transform = if gradient_transform.is_empty() {
		String::new()
	} else {
		format!(r#" gradientTransform="{gradient_transform}""#)
	};

	let gradient_spread = if matches!(settings.spread, GradientSpread::Pad | GradientSpread::Clear) {
		String::new()
	} else {
		format!(r#" spreadMethod="{}""#, settings.spread.svg_name())
	};

	let gradient_id = generate_uuid();

	match gradient_form {
		GradientForm::Linear => {
			let _ = write!(
				svg_defs,
				r#"<linearGradient id="{}" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="1" y2="0"{gradient_spread}{gradient_transform}>{}</linearGradient>"#,
				gradient_id, stop
			);
		}
		GradientForm::Radial => {
			let _ = write!(
				svg_defs,
				r#"<radialGradient id="{}" gradientUnits="userSpaceOnUse" cx="0" cy="0" r="1"{gradient_spread}{gradient_transform}>{}</radialGradient>"#,
				gradient_id, stop
			);
		}
	}

	Some(gradient_id)
}

impl RenderExt for List<Gradient> {
	type Output = Option<u64>;

	/// Adds the gradient def through mutating the first argument, returning the gradient ID, or `None` when the list is empty.
	fn render(
		&self,
		svg_defs: &mut String,
		item_transform: DAffine2,
		element_transform: DAffine2,
		_stroke_transform: DAffine2,
		_bounds: DAffine2,
		render_params: &RenderParams,
		_target: PaintTarget,
	) -> Self::Output {
		render_gradient_paint(
			(!self.is_empty()).then_some(ItemRef::ListItem(self, 0)),
			svg_defs,
			item_transform,
			element_transform,
			render_params.for_mask,
		)
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
		if render_params.stroke_below {
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
			Some(Graphic::Color(item)) => render_color_paint(Some(ItemRef::Item(item)), target, render_params.for_mask),
			Some(Graphic::ColorList(color_list)) => color_list.render(svg_defs, item_transform, element_transform, stroke_transform, bounds, render_params, target),
			Some(Graphic::Gradient(item)) => render_gradient_paint(Some(ItemRef::Item(item)), svg_defs, item_transform, element_transform, render_params.for_mask)
				.map(|gradient_id| format!(r##" {paint_attr}="url(#{gradient_id})""##))
				.unwrap_or_else(|| format!(r#" {paint_attr}="none""#)),
			Some(Graphic::GradientList(gradient_list)) => gradient_list
				.render(svg_defs, item_transform, element_transform, stroke_transform, bounds, render_params, target)
				.map(|gradient_id| format!(r##" {paint_attr}="url(#{gradient_id})""##))
				.unwrap_or_else(|| format!(r#" {paint_attr}="none""#)),
			Some(Graphic::None(_)) | Some(Graphic::NoneList(_)) => format!(r#" {paint_attr}="none""#),
			Some(Graphic::Graphic(_))
			| Some(Graphic::Vector(_))
			| Some(Graphic::RasterCPU(_))
			| Some(Graphic::RasterGPU(_))
			| Some(Graphic::Text(_))
			| Some(Graphic::VectorList(_))
			| Some(Graphic::RasterCPUList(_))
			| Some(Graphic::RasterGPUList(_))
			| Some(Graphic::GraphicList(_))
			| Some(Graphic::TextList(_)) => {
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
