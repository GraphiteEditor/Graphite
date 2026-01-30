use graph_craft::proto::NodeMetadata;
use indoc::indoc;

pub const NODE_CATALOG_PATH: &str = "../../website/content/learn/node-catalog";
pub const OMIT_HIDDEN: bool = true;

pub fn category_description(category: &str) -> &str {
	match category {
		"Animation" => indoc!(
			"
			Nodes in this category enable the creation of animated, real-time, and interactive motion graphics involving paramters that change over time.

			These nodes require that playback is activated by pressing the play button above the viewport.
			"
		),
		"Blending" => "Nodes in this category control how overlapping graphical content is composited together, considering blend modes, opacity, and clipping.",
		"Color" => "Nodes in this category deal with selecting and manipulating colors, gradients, and palettes.",
		"Debug" => indoc!(
			"
			Nodes in this category are temporarily included for debugging purposes by Graphite's developers. They may have rare potential uses for advanced users, but are not intended for general use and will be removed in future releases.
			"
		),
		"General" => "Nodes in this category deal with general data handling, such as merging and flattening graphical elements.",
		"Instancing" => "Nodes in this category enable the duplication, arrangement, and looped generation of graphical elements.",
		"Math: Arithmetic" => "Nodes in this category perform common arithmetic operations on numerical values (and where applicable, `vec2` values).",
		"Math: Logic" => "Nodes in this category perform boolean logic operations such as comparisons, conditionals, logic gates, and switching.",
		"Math: Numeric" => "Nodes in this category perform discontinuous numeric operations such as rounding, clamping, mapping, and randomization.",
		"Math: Transform" => "Nodes in this category perform transformations on graphical elements and calculations involving transformation matrices.",
		"Math: Trig" => "Nodes in this category perform trigonometric operations such as sine, cosine, tangent, and their inverses.",
		"Math: Vector" => "Nodes in this category perform operations involving `vec2` values (points or arrows in 2D space) such as the dot product, normalization, and distance calculations.",
		"Raster: Adjustment" => "Nodes in this category perform per-pixel color adjustments on raster graphics, such as brightness and contrast modifications.",
		"Raster: Channels" => "Nodes in this category enable channel-specific manipulation of the RGB and alpha channels of raster graphics.",
		"Raster: Filter" => "Nodes in this category apply filtering effects to raster graphics such as blurs and sharpening.",
		"Raster: Pattern" => "Nodes in this category generate procedural raster patterns, fractals, textures, and noise.",
		"Raster" => "Nodes in this category deal with fundamental raster image operations.",
		"Text" => "Nodes in this category support the manipulation, formatting, and rendering of text strings.",
		"Value" => "Nodes in this category supply data values of common types such as numbers, colors, booleans, and strings.",
		"Vector: Measure" => "Nodes in this category perform measurements and analysis on vector graphics, such as length/area calculations, path traversal, and hit testing.",
		"Vector: Modifier" => "Nodes in this category modify the geometry of vector graphics, such as boolean operations, smoothing, and morphing.",
		"Vector: Shape" => "Nodes in this category generate parametrically-described primitive vector shapes such as rectangles, grids, stars, and spirals.",
		"Vector: Style" => "Nodes in this category apply fill and stroke styles to alter the appearance of vector graphics.",
		"Vector" => "Nodes in this category deal with fundamental vector graphics data handling and operations.",
		"Web Request" => "Nodes in this category facilitate fetching and handling resources from HTTP endpoints and sending webhook requests to external services.",
		_ => panic!("Category '{category}' is missing a description"),
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

	// Fix letter-number type names
	let mut filtered = format!("-{filtered}-");
	filtered = filtered.replace("-vec-2-", "-vec2-");
	filtered = filtered.replace("-f-32-", "-f32-");
	filtered = filtered.replace("-f-64-", "-f64-");
	filtered = filtered.replace("-u-32-", "-u32-");
	filtered = filtered.replace("-u-64-", "-u64-");
	filtered = filtered.replace("-i-32-", "-i32-");
	filtered = filtered.replace("-i-64-", "-i64-");

	// Remove consecutive dashes
	while filtered.contains("--") {
		filtered = filtered.replace("--", "-");
	}

	// Trim leading and trailing dashes
	filtered.trim_matches('-').to_string()
}
