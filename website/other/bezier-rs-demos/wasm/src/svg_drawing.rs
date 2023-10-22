use glam::DVec2;

// SVG drawing constants
pub const SVG_OPEN_TAG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="250px" height="200px">"#;
pub const SVG_CLOSE_TAG: &str = "</svg>";

// Stylistic constants
pub const BLACK: &str = "black";
pub const WHITE: &str = "white";
pub const GRAY: &str = "gray";
pub const RED: &str = "red";
pub const ORANGE: &str = "orange";
// pub const PINK: &str = "pink";
pub const GREEN: &str = "green";
pub const NONE: &str = "none";

// Default attributes
pub const CURVE_ATTRIBUTES: &str = "stroke=\"black\" stroke-width=\"2\" fill=\"none\"";
pub const HANDLE_LINE_ATTRIBUTES: &str = "stroke=\"gray\" stroke-width=\"1\" fill=\"none\"";
pub const ANCHOR_ATTRIBUTES: &str = "r=\"4\" stroke=\"black\" stroke-width=\"2\" fill=\"white\"";
pub const HANDLE_ATTRIBUTES: &str = "r=\"3\" stroke=\"gray\" stroke-width=\"1.5\" fill=\"white\"";

// Text constants
pub const TEXT_OFFSET_X: f64 = 5.;
pub const TEXT_OFFSET_Y: f64 = 193.;

pub fn wrap_svg_tag(contents: String) -> String {
	format!("{SVG_OPEN_TAG}{contents}{SVG_CLOSE_TAG}")
}

/// Helper function to create an SVG text entity.
pub fn draw_text(text: String, x_pos: f64, y_pos: f64, fill: &str) -> String {
	format!(r#"<text x="{x_pos}" y="{y_pos}" fill="{fill}" font-family="monospace">{text}</text>"#)
}

/// Helper function to create an SVG circle entity.
pub fn draw_circle(position: DVec2, radius: f64, stroke: &str, stroke_width: f64, fill: &str) -> String {
	format!(
		r#"<circle cx="{}" cy="{}" r="{radius}" stroke="{stroke}" stroke-width="{stroke_width}" fill="{fill}"/>"#,
		position.x, position.y
	)
}

/// Helper function to create an SVG circle entity.
pub fn draw_line(start_x: f64, start_y: f64, end_x: f64, end_y: f64, stroke: &str, stroke_width: f64) -> String {
	format!(r#"<line x1="{start_x}" y1="{start_y}" x2="{end_x}" y2="{end_y}" stroke="{stroke}" stroke-width="{stroke_width}"/>"#)
}

// Helper function to convert polar to cartesian coordinates
fn polar_to_cartesian(center_x: f64, center_y: f64, radius: f64, angle_in_rad: f64) -> [f64; 2] {
	let x = center_x + radius * angle_in_rad.cos();
	let y = center_y + radius * -angle_in_rad.sin();
	[x, y]
}

// Helper function to create an SVG drawing of a sector
pub fn draw_sector(center: DVec2, radius: f64, start_angle: f64, end_angle: f64, stroke: &str, stroke_width: f64, fill: &str) -> String {
	let [start_x, start_y] = polar_to_cartesian(center.x, center.y, radius, start_angle);
	let [end_x, end_y] = polar_to_cartesian(center.x, center.y, radius, end_angle);
	// draw sector with fill color
	let sector_svg = format!(
		r#"<path d="M {start_x} {start_y} A {radius} {radius} 0 0 1 {end_x} {end_y} L {} {} L {start_x} {start_y} Z"  stroke="none" fill="{fill}" />"#,
		center.x, center.y
	);
	// draw arc with stroke color
	let arc_svg = format!(r#"<path d="M {start_x} {start_y} A {radius} {radius} 0 0 1 {end_x} {end_y}" stroke="{stroke}" stroke-width="{stroke_width}" fill="none"/>"#);
	format!("{sector_svg}{arc_svg}")
}
