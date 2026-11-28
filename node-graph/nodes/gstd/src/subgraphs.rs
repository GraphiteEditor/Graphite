use crate::artboard::create_artboard;
use crate::context::{read_index, read_vector};
use crate::graphic::{as_graphic, extend, flatten_vector, into_group, item_at_index, map, remove_at_index};
use crate::platform_application_io::{bytes_to_image, load_resource};
use crate::vector::misc::InterpolationDistribution;
use crate::vector::{as_vector, combine_paths, count, morph, path_is_closed, points_to_polyline, separate_paths};
use crate::{Artboard, Color, Graphic, Vector};
use glam::DVec2;
use math_nodes::{add, as_number, divide, floor, max, modulo, multiply, subtract, switch};
use repeat_nodes::repeat;
use transform_nodes::{decompose_translation, extract_transform};

/// Merges the provided content as a new item in the layer's compositing stack.
#[node_macro::graph(category("Graphic"), icon("NodeMerge"), layer)]
pub fn merge(
	/// The stack of layers below this one, which the content is placed above.
	base: List<Graphic>,
	/// The artwork this layer adds to the stack as a single item.
	#[expose]
	content: List<Graphic>,
) -> out!(
	/// The stack of layers with this layer's content placed on top.
	out,
) {
	let grouped = content >> into_group().at(-7, -1);
	base >> as_graphic().at(-7, -2) >> extend(new = grouped).at(0, -2)
}

/// Creates a new Artboard which can be used as a working surface.
#[node_macro::graph(category("Graphic"), icon("NodeArtboard"), layer)]
pub fn artboard(
	/// The stack of artboards below this one, which this artboard is placed above.
	base: List<Artboard>,
	/// Graphics to include within the artboard.
	#[expose]
	content: List<Graphic>,
	/// Coordinate of the top-left corner of the artboard within the document.
	#[labels("X", "Y")]
	#[unit(" px")]
	#[integer]
	location: Item<DVec2>,
	/// Width and height of the artboard within the document.
	#[labels("W", "H")]
	#[unit(" px")]
	#[integer]
	#[default(1920., 1080.)]
	dimensions: Item<DVec2>,
	/// Color of the artboard background.
	#[widget(ParsedWidgetOverride::Custom = "artboard_background")]
	#[default(Color::WHITE)]
	background: Item<Color>,
	/// Whether to cut off the contained content that extends outside the artboard, or keep it visible.
	#[default(true)]
	clip: Item<bool>,
) -> out!(
	/// The stack of artboards with this artboard placed on top.
	out,
) {
	let board = create_artboard(content, location, dimensions, background, clip).at(-7, -3);
	base >> extend(new = board).at(0, -4)
}

/// Creates a series of intermediate shapes that gradually transition between vector objects, optionally along a control path.
#[node_macro::graph(category("Vector: Deform"), icon("NodeBlend"))]
pub fn blend(
	/// The vector objects to create intermediate shapes between.
	content: List<Vector>,
	/// The number of shapes spanning from the first object to the last, repeated for each subpath of the control path.
	#[default(10)]
	count: Item<i64>,
	/// Flip the stacking order of the shapes, so the ones nearest the first object are on top.
	reverse: Item<bool>,
	/// What changes evenly from one shape to the next, which sets how the intermediate shapes are spaced.
	distribution: Item<InterpolationDistribution>,
	/// An optional control path with one anchor point per object. Curved segments shape the trajectory of the blend instead of traveling straight.
	path: Item<Vector>,
) -> out!(
	/// The blended shapes, from the first object to the last.
	out,
) {
	// Each clamps to at least 1, which also covers an empty control path
	let shapes_per_path = count >> as_number().at(2, 13) >> max(other_value = 1).at(9, 13);
	let path_count = path >> separate_paths().at(2, 0) >> count().at(2, 2) >> max(other_value = 1).at(9, 2);

	// Split the repetition index into which subpath it's on and where along that subpath
	let index = read_index().at(2, 7);
	let path_index = index >> divide(denominator = shapes_per_path).at(16, 10) >> floor().at(23, 4);
	let local_index = index >> modulo(modulus = shapes_per_path).at(16, 7);

	// A closed subpath spaces its shapes by the count, and an open one by one fewer (at least 1) so its last shape lands on the end
	let open_spacing = shapes_per_path >> subtract(subtrahend = 1).at(16, 14) >> max(other_value = 1).at(23, 14);
	let spacing = path >> path_is_closed(index = path_index).at(30, 2) >> switch(if_true = shapes_per_path, if_false = open_spacing).at(30, 12);

	// Scaling just below 1 keeps the last shape from overflowing into the next subpath
	let fraction = local_index >> divide(denominator = spacing).at(23, 7) >> multiply(multiplicand = 0.9999999999).at(30, 7);
	let progression = path_index >> add(addend = fraction).at(37, 4);

	let total = shapes_per_path >> multiply(multiplicand = path_count).at(16, 1);
	content >> morph(progression, distribution, path).at(44, 3) >> repeat(count = total, reverse).at(51, 0)
}

/// Connects the origins of vector objects in order with an open polyline, such as to make a control path for the **Blend** and **Morph** nodes.
#[node_macro::graph(category("Vector: Path"), icon("NodeOriginsToPolyline"), name("Origins to Polyline"))]
pub fn origins_to_polyline(
	/// The vector objects to connect in order by their origins.
	vector: List<Vector>,
) -> out!(
	/// An open polyline with an anchor point at the origin of each object.
	vector,
) {
	// Each object's origin, read from the item being mapped, as a single-anchor vector
	let origin = read_vector().at(0, 1) >> extract_transform().at(7, 1) >> decompose_translation().at(14, 1) >> as_vector().at(21, 1);

	vector >> flatten_vector().at(21, 0) >> map(mapped = origin).at(28, 0) >> combine_paths().at(35, 0) >> points_to_polyline(closed = false).at(42, 0)
}

/// Loads an image from a given URL.
#[node_macro::graph(category("Data: I/O"), icon("NodeLoadImage"))]
pub fn load_image(
	_: (),
	/// The web address or local asset path of the image to load.
	#[name("URL")]
	url: Item<String>,
) -> out!(
	/// The loaded image.
	image,
) {
	load_resource(url).at(0, 0) >> bytes_to_image().at(7, 0)
}

/// Finds a portion of the string matching a regular expression pattern. With "Match Index" at its default 0, it selects the first non-overlapping occurrence, but others may be selected. Capture groups, if any, are produced as a list in the "Captures" output.
#[node_macro::graph(category("String: Regex"), icon("NodeRegexFind"))]
pub fn regex_find(
	/// The string to search within.
	string: Item<String>,
	/// The regular expression pattern to search for.
	pattern: Item<String>,
	/// Which non-overlapping occurrence of the pattern to return, starting from 0 for the first match. Negative indices count backwards from the last match.
	match_index: Item<i64>,
	/// Match letters regardless of case.
	case_insensitive: Item<bool>,
	/// Make `^` and `$` match the start and end of each line, not just the whole string.
	multiline: Item<bool>,
) -> out!(
	/// The whole portion of the string matching the pattern, or empty if there is no match.
	r#match,
	/// The text matched by each capture group in the pattern, in order.
	captures,
) {
	// The whole match, then each capture group, where each item carries `start`, `end`, and `name` attributes
	let found = string >> text_nodes::regex::regex_find(pattern, match_index, case_insensitive, multiline).at(0, 0);

	let whole_match = found >> item_at_index(index = 0).at(8, 0);
	let capture_groups = found >> remove_at_index(index = 0).at(8, 2);

	(whole_match, capture_groups)
}

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::registry::GraphInputSource;

	/// Finds the capture groups of a pattern.
	#[node_macro::graph(category("Test"))]
	fn regex_captures(string: Item<String>, pattern: Item<String>) -> out!(captures) {
		return string >> regex_find(pattern).captures;
	}

	#[test]
	fn an_output_chosen_by_name_wires_from_that_output() {
		let graph = _regex_captures_graph_metadata();
		assert!(matches!(graph.exports[0].source, GraphInputSource::Node { node: 0, output: 1 }));
	}

	/// Puts its content on a board, setting every piece of node state.
	#[node_macro::graph(category("Test"))]
	fn boarded(content: List<Graphic>) -> out!(out) {
		return create_artboard(content, background = rgb(0x102030), clip = exposed(false))
			.named("Board")
			.hidden()
			.locked()
			.pinned()
			.layer()
			.call_argument::<impl Any>()
			.at(3, -4);
	}

	#[test]
	fn node_state_and_values_reach_the_metadata() {
		use core_types::NodeParameter;
		use core_types::registry::GraphValue;

		let graph = _boarded_graph_metadata();
		let node = &graph.nodes[0];
		assert_eq!(node.display_name, Some("Board"));
		assert!(node.hidden && node.locked && node.pinned && node.layer);
		assert_eq!(node.position, Some((3, -4)));
		assert!(matches!(node.call_argument, Some(core_types::Type::Generic(_))));

		let source = |index: usize| &node.inputs.iter().find(|(input_index, _)| *input_index == index).expect("the input should be set").1;
		assert!(matches!(source(create_artboard::ContentInput::INDEX), GraphInputSource::Import { index: 0, .. }));
		assert!(matches!(
			source(create_artboard::BackgroundInput::INDEX),
			GraphInputSource::Value {
				value: GraphValue::Color(_),
				exposed: None
			}
		));
		assert!(matches!(
			source(create_artboard::ClipInput::INDEX),
			GraphInputSource::Value {
				value: GraphValue::Bool(false),
				exposed: Some(true)
			}
		));
	}
}
