use std::collections::HashMap;

use core_types::{
	ATTR_GRADIENT_FORM, ATTR_GRADIENT_SPACE, ATTR_GRADIENT_SPREAD, Color,
	list::{ATTR_FILL, ATTR_STROKE, ATTR_TRANSFORM, Item, List},
};
use glam::{DAffine2, DVec2};
use graphic_types::graphic::set_paint_attribute_at;
use graphic_types::{Graphic, IntoGraphicList};
use log::warn;
use vector_types::{
	Vector,
	subpath::{ManipulatorGroup, Subpath},
	vector::{
		PointId,
		style::{Gradient, GradientForm, GradientSpace, GradientSpread, GradientStop, PaintOrder, Stroke, StrokeAlign, StrokeCap, StrokeJoin},
	},
	vectorize_config,
};

pub fn convert_usvg_path(path: &usvg::Path) -> Vec<Subpath<PointId>> {
	let mut subpaths = Vec::new();
	let mut manipulators_list = Vec::new();

	let mut points = path.data().points().iter();
	let to_vec = |p: &usvg::tiny_skia_path::Point| DVec2::new(p.x as f64, p.y as f64);

	for verb in path.data().verbs() {
		match verb {
			usvg::tiny_skia_path::PathVerb::Move => {
				subpaths.push(Subpath::new(std::mem::take(&mut manipulators_list), false));
				let Some(start) = points.next().map(to_vec) else { continue };
				manipulators_list.push(ManipulatorGroup::new(start, Some(start), Some(start)));
			}
			usvg::tiny_skia_path::PathVerb::Line => {
				let Some(end) = points.next().map(to_vec) else { continue };
				manipulators_list.push(ManipulatorGroup::new(end, Some(end), Some(end)));
			}
			usvg::tiny_skia_path::PathVerb::Quad => {
				let Some(handle) = points.next().map(to_vec) else { continue };
				let Some(end) = points.next().map(to_vec) else { continue };
				if let Some(last) = manipulators_list.last_mut() {
					last.out_handle = Some(last.anchor + (2. / 3.) * (handle - last.anchor));
				}
				manipulators_list.push(ManipulatorGroup::new(end, Some(end + (2. / 3.) * (handle - end)), Some(end)));
			}
			usvg::tiny_skia_path::PathVerb::Cubic => {
				let Some(first_handle) = points.next().map(to_vec) else { continue };
				let Some(second_handle) = points.next().map(to_vec) else { continue };
				let Some(end) = points.next().map(to_vec) else { continue };
				if let Some(last) = manipulators_list.last_mut() {
					last.out_handle = Some(first_handle);
				}
				manipulators_list.push(ManipulatorGroup::new(end, Some(second_handle), Some(end)));
			}
			usvg::tiny_skia_path::PathVerb::Close => {
				subpaths.push(Subpath::new(std::mem::take(&mut manipulators_list), true));
			}
		}
	}
	subpaths.push(Subpath::new(manipulators_list, false));
	subpaths
}

pub fn convert_gradient_spread(spread_method: usvg::SpreadMethod) -> GradientSpread {
	match spread_method {
		usvg::SpreadMethod::Pad => GradientSpread::Pad,
		usvg::SpreadMethod::Reflect => GradientSpread::Reflect,
		usvg::SpreadMethod::Repeat => GradientSpread::Repeat,
	}
}

fn usvg_color(c: usvg::Color, a: f32) -> Color {
	// `usvg::Color` channels are u8 sRGB display values (gamma-encoded); lift to linear-light for the internal `Color`
	Color::from_gamma_srgb_channels(c.red as f32 / 255., c.green as f32 / 255., c.blue as f32 / 255., a)
}

pub fn usvg_transform(c: usvg::Transform) -> DAffine2 {
	DAffine2::from_cols_array(&[c.sx as f64, c.ky as f64, c.kx as f64, c.sy as f64, c.tx as f64, c.ty as f64])
}

const GRAPHITE_NAMESPACE: &str = "https://graphite.art";

/// Gradient information pre-parsed from the raw SVG XML, carrying what usvg's simplified tree drops.
pub struct SvgGradientInfo {
	/// Real stops, keyed by gradient element `id`, for gradients Graphite exported with midpoint curve data.
	pub graphite_stops: HashMap<String, Gradient>,
	/// Gradient spaces, keyed by gradient element `id`, resolved from the `color-interpolation` property.
	pub spaces: HashMap<String, GradientSpace>,
}

/// Pre-parses the raw SVG XML to resolve each gradient's inherited `color-interpolation` property, which usvg's
/// tree does not carry. Only `linearRGB` selects the linear space; `auto` and `sRGB` (browsers treat the
/// user-agent-defined `auto` as `sRGB`) mean gamma, as does any unrecognized value.
pub fn extract_gradient_spaces(svg: &str) -> HashMap<String, GradientSpace> {
	let mut result = HashMap::new();

	// Quick check: gradients in an SVG that never mentions `color-interpolation` all take the sRGB default
	if !svg.contains("color-interpolation") {
		return result;
	}

	let doc = match usvg::roxmltree::Document::parse(svg) {
		Ok(doc) => doc,
		Err(_) => return result,
	};

	// The document's `<style>` blocks apply to every element, so parse them once up front
	let mut stylesheet = simplecss::StyleSheet::new();
	for style_element in doc.descendants().filter(|node| node.tag_name().name() == "style") {
		if !matches!(style_element.attribute("type"), None | Some("") | Some("text/css")) {
			continue;
		}
		for text in style_element.children().filter(|child| child.is_text()).filter_map(|child| child.text()) {
			stylesheet.parse_more(text);
		}
	}

	for node in doc.descendants() {
		match node.tag_name().name() {
			"linearGradient" | "radialGradient" => {}
			_ => continue,
		}

		if let Some(gradient_id) = node.attribute("id")
			&& let Some(gradient_space) = resolve_color_interpolation(node, &stylesheet)
		{
			result.insert(gradient_id.to_string(), gradient_space);
		}
	}

	result
}

/// The `color-interpolation` in effect for an element: the nearest self-or-ancestor declaration, taking each
/// element's own winning declaration per [`declared_color_interpolation`]'s cascade order.
pub fn resolve_color_interpolation(element: usvg::roxmltree::Node, stylesheet: &simplecss::StyleSheet) -> Option<GradientSpace> {
	let mut next = Some(element);

	while let Some(element) = next {
		match declared_color_interpolation(element, stylesheet) {
			Some("linearRGB") => return Some(GradientSpace::RgbLinear),
			// `inherit` defers to the ancestors like an undeclared element
			Some("inherit") | None => {}
			Some(_) => return Some(GradientSpace::RgbGamma),
		}

		next = element.parent_element();
	}

	None
}

/// The winning `color-interpolation` declaration on a single element per the CSS cascade: `!important` declarations
/// beat normal ones, the inline `style` beats the `<style>` rules (already specificity-sorted, so their last match
/// wins), and the presentation attribute yields to them all. Later declarations win priority ties.
pub fn declared_color_interpolation<'a>(element: usvg::roxmltree::Node<'a, '_>, stylesheet: &simplecss::StyleSheet<'a>) -> Option<&'a str> {
	let mut winner: Option<(u8, &'a str)> = None;
	let mut consider = |priority: u8, value: &'a str| {
		if winner.is_none_or(|(existing, _)| priority >= existing) {
			winner = Some((priority, value));
		}
	};

	if let Some(value) = element.attribute("color-interpolation") {
		consider(0, value.trim());
	}

	for rule in stylesheet.rules.iter().filter(|rule| rule.selector.matches(&CssElement(element))) {
		for declaration in rule.declarations.iter().filter(|declaration| declaration.name == "color-interpolation") {
			consider(if declaration.important { 3 } else { 1 }, declaration.value);
		}
	}

	if let Some(style) = element.attribute("style") {
		for declaration in simplecss::DeclarationTokenizer::from(style).filter(|declaration| declaration.name == "color-interpolation") {
			consider(if declaration.important { 4 } else { 2 }, declaration.value);
		}
	}

	winner.map(|(_, value)| value)
}

/// Adapts a roxmltree element to simplecss's selector-matching interface.
struct CssElement<'a, 'input>(usvg::roxmltree::Node<'a, 'input>);

impl simplecss::Element for CssElement<'_, '_> {
	fn parent_element(&self) -> Option<Self> {
		self.0.parent_element().map(CssElement)
	}

	fn prev_sibling_element(&self) -> Option<Self> {
		self.0.prev_sibling_element().map(CssElement)
	}

	fn has_local_name(&self, local_name: &str) -> bool {
		self.0.tag_name().name() == local_name
	}

	fn attribute_matches(&self, local_name: &str, operator: simplecss::AttributeOperator) -> bool {
		self.0.attribute(local_name).is_some_and(|value| operator.matches(value))
	}

	fn pseudo_class_matches(&self, class: simplecss::PseudoClass) -> bool {
		matches!(class, simplecss::PseudoClass::FirstChild) && self.0.prev_sibling_element().is_none()
	}
}

// Pre-parses the raw SVG XML to extract gradient stops that have `graphite:midpoint` attributes.
// Graphite exports gradients with midpoint curve data by writing interpolated approximation stops
// alongside the real stops. Real stops are tagged with `graphite:midpoint` attributes.
// Returns a map from gradient element `id` to `Gradient` containing only the real stops.
pub fn extract_graphite_gradient_stops(svg: &str) -> HashMap<String, Gradient> {
	let mut result = HashMap::new();

	// Quick check: if the SVG doesn't reference `graphite:midpoint` at all, skip parsing
	if !svg.contains("graphite:midpoint") {
		return result;
	}

	let doc = match usvg::roxmltree::Document::parse(svg) {
		Ok(doc) => doc,
		Err(_) => return result,
	};

	for node in doc.descendants() {
		match node.tag_name().name() {
			"linearGradient" | "radialGradient" => {}
			_ => continue,
		}

		let gradient_id = match node.attribute("id") {
			Some(id) => id.to_string(),
			None => continue,
		};

		let mut real_stops = Vec::new();
		let mut has_any_midpoint = false;

		for child in node.children() {
			if child.tag_name().name() != "stop" {
				continue;
			}

			let midpoint = child.attribute((GRAPHITE_NAMESPACE, "midpoint")).and_then(|v| v.parse::<f64>().ok());

			if let Some(midpoint) = midpoint {
				has_any_midpoint = true;

				let offset = child.attribute("offset").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.);
				let opacity = child.attribute("stop-opacity").and_then(|v| v.parse::<f32>().ok()).unwrap_or(1.);
				let color = child.attribute("stop-color").and_then(|hex| parse_hex_stop_color(hex, opacity)).unwrap_or(Color::BLACK);

				real_stops.push(GradientStop { position: offset, midpoint, color });
			}
		}

		if has_any_midpoint && !real_stops.is_empty() {
			result.insert(gradient_id, Gradient::new(real_stops));
		}
	}

	result
}

pub fn parse_hex_stop_color(hex: &str, opacity: f32) -> Option<Color> {
	let hex = hex.strip_prefix('#')?;
	if hex.len() != 6 {
		return None;
	}
	let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.;
	let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.;
	let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.;
	Some(Color::from_rgbaf32_unchecked(r, g, b, opacity))
}

// Create an intermidate representation that holds data extracted from usvg data structures
// Rewrite all the functions below to be independent of the ModifyInputsContext data structure
// These functions should be able to convert data from usvg into Graphite internal data structures (Fill, Stroke, Vector)
// Use that functions to do the same functions as importing an svg as well as for implementing the Vectorize node
// Vectorize node should also be able to insert a fill, stroke or path node according to the resulting SVG data from vtracer
// Implement tooling in adjacent to Vectorize node to be able to insert Fill, Stroke or Text nodes into the node graph
pub enum ParsedSvgNode {
	Group(Box<ParsedSvgGroup>),
	Path(Box<ParsedSvgPath>),
	Text(Box<ParsedSvgText>),
	Image { msg: String },
}

pub struct ParsedSvgGroup {
	pub children: Vec<ParsedSvgNode>,
	pub transform: DAffine2,
}

pub struct ParsedSvgPath {
	pub subpaths: Vec<Subpath<PointId>>,
	pub fill_paint: Option<List<Graphic>>,
	pub stroke: Option<Stroke>,
	pub stroke_paint: Option<List<Graphic>>,
	pub transform: DAffine2,
}

pub struct ParsedSvgText {
	pub text: String,
	pub transform: DAffine2,
}

/// Extract fill paint from a usvg fill. Only solid colors are supported for now.
pub fn extract_usvg_fill(fill: &usvg::Fill, gradient_info: &SvgGradientInfo) -> Option<List<Graphic>> {
	match &fill.paint() {
		usvg::Paint::Color(color) => {
			let color = usvg_color(*color, fill.opacity().get());
			Some(List::new_from_element(color).into_graphic_list())
		}
		usvg::Paint::LinearGradient(linear) => {
			let gradient_transform = usvg_transform(linear.transform());
			let (start, end) = (DVec2::new(linear.x1() as f64, linear.y1() as f64), DVec2::new(linear.x2() as f64, linear.y2() as f64));
			let (start, end) = (gradient_transform.transform_point2(start), gradient_transform.transform_point2(end));
			let direction = end - start;
			let transform = DAffine2::from_cols(direction, direction.perp(), start);

			let gradient_form = GradientForm::Linear;

			let gradient = match gradient_info.graphite_stops.get(linear.id()) {
				Some(graphite_stops) => graphite_stops.clone(),
				None => {
					let stops = linear.stops().iter().map(|stop| GradientStop {
						position: stop.offset().get() as f64,
						midpoint: 0.5,
						color: usvg_color(stop.color(), stop.opacity().get()),
					});
					Gradient::new(stops)
				}
			};
			let gradient_spread = convert_gradient_spread(linear.spread_method());
			// SVG interpolates between stops in gamma sRGB unless `color-interpolation` opts into linearRGB, carried explicitly rather than as the linear default
			let gradient_space = gradient_info.spaces.get(linear.id()).copied().unwrap_or(GradientSpace::RgbGamma);

			let gradient = Item::new_from_element(gradient)
				.with_attribute(ATTR_GRADIENT_FORM, gradient_form)
				.with_attribute(ATTR_GRADIENT_SPREAD, gradient_spread)
				.with_attribute(ATTR_GRADIENT_SPACE, gradient_space)
				.with_attribute(ATTR_TRANSFORM, transform);
			Some(List::new_from_item(gradient).into_graphic_list())
		}
		usvg::Paint::RadialGradient(radial) => {
			let gradient_transform = usvg_transform(radial.transform());
			let center = DVec2::new(radial.cx() as f64, radial.cy() as f64);
			let edge = center + DVec2::X * radial.r().get() as f64;
			let (start, end) = (gradient_transform.transform_point2(center), gradient_transform.transform_point2(edge));
			let direction = end - start;
			let transform = DAffine2::from_cols(direction, direction.perp(), start);

			let gradient_form = GradientForm::Radial;

			let gradient = match gradient_info.graphite_stops.get(radial.id()) {
				Some(graphite_stops) => graphite_stops.clone(),
				None => {
					let stops = radial.stops().iter().map(|stop| GradientStop {
						position: stop.offset().get() as f64,
						midpoint: 0.5,
						color: usvg_color(stop.color(), stop.opacity().get()),
					});
					Gradient::new(stops)
				}
			};
			let gradient_spread = convert_gradient_spread(radial.spread_method());
			let gradient_space = gradient_info.spaces.get(radial.id()).copied().unwrap_or(GradientSpace::RgbGamma);

			let gradient = Item::new_from_element(gradient)
				.with_attribute(ATTR_GRADIENT_FORM, gradient_form)
				.with_attribute(ATTR_GRADIENT_SPREAD, gradient_spread)
				.with_attribute(ATTR_GRADIENT_SPACE, gradient_space)
				.with_attribute(ATTR_TRANSFORM, transform);
			Some(List::new_from_item(gradient).into_graphic_list())
		}
		usvg::Paint::Pattern(_) => {
			warn!("SVG patterns are not currently supported");
			None
		}
	}
}

/// Extract stroke and stroke paint from a usvg stroke.
/// Returns (stroke, stroke_paint).
pub fn extract_usvg_stroke(stroke: &usvg::Stroke, transform: DAffine2) -> (Option<Stroke>, Option<List<Graphic>>) {
	let graphite_color = match &stroke.paint() {
		usvg::Paint::Color(color) => Some(usvg_color(*color, stroke.opacity().get())),
		_ => None,
	};

	let weight = stroke.width().get() as f64;
	if weight <= 0. {
		return (None, None);
	}

	let stroke = Stroke {
		weight,
		dash_lengths: stroke.dasharray().as_ref().map(|lengths| lengths.iter().map(|&length| length as f64).collect()).unwrap_or_default(),
		dash_offset: stroke.dashoffset() as f64,
		cap: match stroke.linecap() {
			usvg::LineCap::Butt => StrokeCap::Butt,
			usvg::LineCap::Round => StrokeCap::Round,
			usvg::LineCap::Square => StrokeCap::Square,
		},
		join: match stroke.linejoin() {
			usvg::LineJoin::Miter | usvg::LineJoin::MiterClip => StrokeJoin::Miter,
			usvg::LineJoin::Round => StrokeJoin::Round,
			usvg::LineJoin::Bevel => StrokeJoin::Bevel,
		},
		join_miter_limit: stroke.miterlimit().get() as f64,
		align: StrokeAlign::Center,
		paint_order: PaintOrder::StrokeAbove,
		transform,
	};

	let paint = graphite_color.map(|c| List::new_from_element(c).into_graphic_list());
	(Some(stroke), paint)
}

pub fn extract_usvg_path(node: &usvg::Node, path: &usvg::Path, gradient_info: &SvgGradientInfo) -> ParsedSvgPath {
	let subpaths = convert_usvg_path(path);
	let transform = usvg_transform(node.abs_transform());

	let (stroke, stroke_paint) = path.stroke().map(|s| extract_usvg_stroke(s, transform)).unwrap_or((None, None));

	ParsedSvgPath {
		subpaths,
		fill_paint: path.fill().and_then(|fill| extract_usvg_fill(fill, gradient_info)),
		stroke,
		stroke_paint,
		transform,
	}
}

pub fn extract_usvg_node(node: &usvg::Node, gradient_info: &SvgGradientInfo) -> ParsedSvgNode {
	match node {
		usvg::Node::Group(group) => {
			let group = Box::new(ParsedSvgGroup {
				children: group.children().iter().map(|child| extract_usvg_node(child, gradient_info)).collect(),
				transform: usvg_transform(node.abs_transform()),
			});

			ParsedSvgNode::Group(group)
		}
		usvg::Node::Path(path) => ParsedSvgNode::Path(Box::new(extract_usvg_path(node, path, gradient_info))),
		usvg::Node::Image(_) => ParsedSvgNode::Image { msg: String::from("Not supported") },
		usvg::Node::Text(text) => {
			let text = ParsedSvgText {
				text: text.chunks().iter().map(|c| c.text()).collect(),
				transform: usvg_transform(node.abs_transform()),
			};
			ParsedSvgNode::Text(Box::new(text))
		}
	}
}

pub fn extract_all_paths(
	parsed_node: ParsedSvgNode,
	vectors: &mut List<Vector>,
	vectorize_mode: vectorize_config::VectorizeMode,
	//  mut i: i32
) {
	match parsed_node {
		ParsedSvgNode::Group(group) => {
			for child in group.children {
				// if let ParsedSvgNode::Path(ref path) = child {
				// 	log::debug!("Reading path (in a group) {} from a total of {}.", i, svg_tree.root().children().len());
				// 	i += 1;
				// }
				extract_all_paths(child, vectors, vectorize_mode);
			}
		}
		ParsedSvgNode::Path(path) => {
			let mut child_subpaths = path.subpaths.clone();
			child_subpaths.iter_mut().for_each(|s| s.apply_transform(path.transform));
			let mut vector = Vector::from_subpaths(child_subpaths, false);

			if let vectorize_config::VectorizeMode::FullImage = vectorize_mode {
				vector.stroke = path.stroke;
			}

			let index = vectors.len();
			vectors.push(Item::new_from_element(vector));
			if let vectorize_config::VectorizeMode::FullImage = vectorize_mode {
				if let Some(fill_paint) = path.fill_paint {
					set_paint_attribute_at(vectors, index, ATTR_FILL, fill_paint);
				}
				if let Some(stroke_paint) = path.stroke_paint {
					set_paint_attribute_at(vectors, index, ATTR_STROKE, stroke_paint);
				}
			}
			// log::debug!("Reading path {} from a total of {}.", i, svg_tree.root().children().len());
			// i += 1;
		}
		_ => {}
	}
}
