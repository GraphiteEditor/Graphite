use graph_craft::proto::NodeMetadata;
use indoc::indoc;

pub const OMIT_HIDDEN: bool = true;

pub fn category_description(category: &str) -> &str {
	match category {
		"Value" => "Nodes in this category supply data values of common types such as numbers, colors, gradients, booleans, and strings.",
		"Routing" => "Nodes in this category direct how data flows through the graph, such as switching between inputs, wrapping nodes into a custom subgraph, and caching results.",
		"Transform" => "Nodes in this category move, rotate, scale, and skew graphical content, read and combine the transformation matrices that place it, and arrange items by packing them into rows or columns.",
		"Graphic" => "Nodes in this category compose graphical content of any kind, such as merging layers, making artboards and groups, blending and clipping, repeating and mirroring copies, and flattening nested groups.",
		"Vector: Style" => "Nodes in this category apply fill and stroke styles to alter the appearance of vector graphics.",
		"Vector: Geometry" => "Nodes in this category create vector geometry, either drawn by hand with the **Path** node or generated from parameters as shapes such as rectangles, stars, spirals, grids, and QR codes.",
		"Vector: Point" => "Nodes in this category generate, move, and connect the points of vector graphics, such as scattering, jittering, and relaxing them, or building triangulations and Voronoi cells from them.",
		"Vector: Path" => "Nodes in this category reshape the paths of vector graphics, such as boolean operations, smoothing and rounding corners, offsetting and simplifying, and joining, splitting, closing, and reversing paths.",
		"Vector: Deform" => "Nodes in this category distort or interpolate whole vector shapes, such as morphing and blending between shapes, extruding, and warping within a box.",
		"Vector: Measure" => "Nodes in this category perform measurements and analysis on vector graphics, such as length/area calculations, path traversal, and hit testing.",
		"Raster" => "Nodes in this category create and combine raster images, such as loading image files, rasterizing graphics, painting brush strokes, generating noise and fractal patterns, masking, and splitting or merging color channels.",
		"Raster: Effect" => "Nodes in this category add generated effects to raster graphics, such as color overlays.",
		"Raster: Adjustment" => "Nodes in this category perform per-pixel color adjustments on raster graphics, such as brightness and contrast modifications.",
		"Raster: Filter" => "Nodes in this category apply filtering effects to raster graphics, such as blurring, median filtering, and dehazing.",
		"Color" => "Nodes in this category build colors from their channels or from text, and pick colors out of gradients and images.",
		"Gradient" => "Nodes in this category build gradients and adjust the placement of their color stops, as well as the shape and repetition of the transition between them.",
		"Typography" => "Nodes in this category lay out and render text in a chosen font, either as text or converted into vector shapes.",
		"String" => "Nodes in this category edit, combine, split, and search strings of text.",
		"String: Regex" => "Nodes in this category perform string operations involving regular expressions, such as pattern matching and replacement.",
		"String: JSON" => "Nodes in this category perform string operations involving JSON data, such as querying and formatting.",
		"Math" => "Nodes in this category evaluate math expressions, convert between numbers and text, and generate random numbers.",
		"Math: Arithmetic" => "Nodes in this category perform common arithmetic operations on numerical values (and where applicable, `vec2` values).",
		"Math: Logic" => "Nodes in this category perform boolean logic operations such as comparisons and logic gates.",
		"Math: Numeric" => "Nodes in this category perform numeric operations such as rounding, clamping, finding minimums and maximums, remapping between ranges, and interpolating.",
		"Math: Trig" => "Nodes in this category perform trigonometric operations such as sine, cosine, tangent, and their inverses.",
		"Math: Vec2" => "Nodes in this category perform operations involving `vec2` values (points or arrows in 2D space) such as the dot product, normalization, and distance calculations.",
		"Data: List" => "Nodes in this category build, reorder, filter, and pick from lists of items, and loop over them to transform each one.",
		"Data: Context" => "Nodes in this category read data from the evaluation context supplied by downstream nodes, such as the current loop iteration index.",
		"Data: Animation" => indoc!(
			"
			Nodes in this category enable the creation of animated, real-time, and interactive motion graphics involving parameters that change over time.

			These nodes require that playback is activated by pressing the play button above the viewport.
			"
		),
		"Data: Attribute" => "Nodes in this category read and write the named attributes stored on the items of a list, with values calculated individually for each item.",
		"Data: Type Assertion" => "Nodes in this category assert that a value is of a specific data type, which may be used to invoke automatic conversions to the desired type.",
		"Data: I/O" => "Nodes in this category bring data into and out of the graph, such as sending web requests, loading images and other files, converting between bytes and images or text, and logging values to the console.",
		_ => {
			eprintln!("Category '{category}' is missing a description");

			"*Category description coming soon.*"
		}
	}.trim()
}

pub fn node_description(metadata: &NodeMetadata) -> &str {
	let mut description = metadata.description.trim();
	if description.is_empty() {
		description = "*Node description coming soon.*";
	}
	description
}

pub fn sanitize_path(s: &str) -> String {
	// Replace disallowed characters with a dash
	let allowed_characters = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~[]@!$&'()*+,;=";
	let filtered = s.chars().map(|c| if allowed_characters.contains(c) { c } else { '-' }).collect::<String>();

	// Rejoin split abbreviations
	let mut filtered = format!("-{filtered}-");
	filtered = filtered.replace("-vec-2-", "-vec2-");
	filtered = filtered.replace("-f-32-", "-f32-");
	filtered = filtered.replace("-f-64-", "-f64-");
	filtered = filtered.replace("-u-32-", "-u32-");
	filtered = filtered.replace("-u-64-", "-u64-");
	filtered = filtered.replace("-i-32-", "-i32-");
	filtered = filtered.replace("-i-64-", "-i64-");
	filtered = filtered.replace("-i-o-", "-io-");

	// Remove consecutive dashes
	while filtered.contains("--") {
		filtered = filtered.replace("--", "-");
	}

	// Trim leading and trailing dashes
	filtered.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn every_catalog_category_has_a_description() {
		let missing: Vec<_> = graphene_std::registry::CATEGORY_ORDER
			.iter()
			.filter(|category| category_description(category) == "*Category description coming soon.*")
			.collect();
		assert!(missing.is_empty(), "categories missing a description: {missing:?}");
	}
}
