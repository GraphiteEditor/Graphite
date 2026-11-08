use super::*;
use crate::pdf_scene_builder::PdfSceneBuilder;
use crate::scene_builder::SceneBuilder;
use crate::{RenderParams, RenderToScene};
use core_types::Color;
use core_types::list::{ATTR_APPEARANCE, ATTR_PATHS, Item, List};
use graphic_types::raster_types::{Image, Raster};
use graphic_types::vector_types::vector::style::Stroke as GraphiteStroke;
use graphic_types::{Appearance, CoverPlacement, Coverage, Graphic, Vector};
use kurbo::{Circle, Line, Size};
use vello_gpu::peniko::BlendMode;

const RED: peniko::Color = peniko::Color::from_rgb8(255, 0, 0);
const BLUE: peniko::Color = peniko::Color::from_rgb8(0, 0, 255);

/// A page of 192 by 96 pixels drawn by `draw`, encoded at 96 pixels per inch.
fn encode(draw: impl FnOnce(&mut PdfSceneBuilder)) -> Vec<u8> {
	let mut scene = PdfSceneBuilder::new(Size::new(192., 96.), Affine::IDENTITY);
	draw(&mut scene);
	scene.finish().encode_pdf(96.).expect("the page encodes")
}

/// The dictionaries and decoded streams of every object in a PDF, which also checks that it parses.
fn pdf_text(pdf: &[u8]) -> String {
	use hayro_syntax::object::Object;

	let pdf = hayro_syntax::Pdf::new(pdf.to_vec()).expect("the PDF parses");
	let mut text = String::new();
	for object in pdf.objects() {
		match object {
			Object::Dict(dict) => text.push_str(&String::from_utf8_lossy(dict.data())),
			Object::Stream(stream) => {
				text.push_str(&String::from_utf8_lossy(stream.dict().data()));
				text.push_str(&String::from_utf8_lossy(&stream.decoded().expect("the stream decodes")));
			}
			_ => {}
		}
		text.push('\n');
	}
	text
}

fn gradient(kind: impl Into<GradientKind>, extend: Extend, stops: &[(f32, peniko::Color)]) -> peniko::Gradient {
	let mut gradient = peniko::Gradient {
		kind: kind.into(),
		extend,
		interpolation_alpha_space: InterpolationAlphaSpace::Unpremultiplied,
		..Default::default()
	};
	gradient.stops.0.extend(stops.iter().map(|&(offset, color)| peniko::ColorStop::from((offset, color))));
	gradient
}

fn assert_close(actual: f64, expected: f64) {
	assert!((actual - expected).abs() < 1e-6, "expected {expected}, got {actual}");
}

#[test]
fn pages_are_sized_in_points_by_the_pixels_per_inch() {
	let scene = PdfSceneBuilder::new(Size::new(192., 96.), Affine::IDENTITY).finish();

	for (pixels_per_inch, width, height) in [(96., 144., 72.), (72., 192., 96.), (300., 46.08, 23.04)] {
		let pdf = scene.encode_pdf(pixels_per_inch).expect("the page encodes");
		let pdf = hayro_syntax::Pdf::new(pdf).expect("the PDF parses");
		let media_box = pdf.pages()[0].media_box();
		assert!((media_box.x1 - media_box.x0 - width).abs() < 1e-3, "{media_box:?}");
		assert!((media_box.y1 - media_box.y0 - height).abs() < 1e-3, "{media_box:?}");
	}

	assert!(scene.encode_pdf(0.).is_err());
	assert!(PdfSceneBuilder::new(Size::new(0., 96.), Affine::IDENTITY).finish().encode_pdf(96.).is_err());
}

#[test]
fn each_page_shows_its_own_region_of_the_scene() {
	let mut scene = PdfSceneBuilder::new(Size::new(300., 100.), Affine::IDENTITY);
	scene.fill(Fill::NonZero, Affine::IDENTITY, RED, None, &Rect::new(10., 10., 90., 90.));
	scene.fill(Fill::NonZero, Affine::IDENTITY, BLUE, None, &Rect::new(210., 10., 290., 70.));
	let scene = scene.finish();

	let page = |x: f64, (width, height): (f64, f64), bookmark: &str| PdfPage {
		origin: Point::new(x, 0.),
		size: Size::new(width, height),
		bookmark: Some(bookmark.to_string()),
	};
	let pdf = scene
		.encode_pdf_pages(&[page(0., (100., 100.), "First"), page(200., (100., 80.), "Second")], 96.)
		.expect("the pages encode");
	let text = pdf_text(&pdf);
	let pdf = hayro_syntax::Pdf::new(pdf).expect("the PDF parses");
	let pages = pdf.pages();
	assert_eq!(pages.len(), 2);

	// Each page is its region's size in points, drawing only what's within it, moved to put the region's corner at its origin
	for (page, (width, height), color, other_color) in [(&pages[0], (75., 75.), "1 0 0 rg", "0 0 1 rg"), (&pages[1], (75., 60.), "0 0 1 rg", "1 0 0 rg")] {
		let media_box = page.media_box();
		assert!((media_box.x1 - media_box.x0 - width).abs() < 1e-3, "{media_box:?}");
		assert!((media_box.y1 - media_box.y0 - height).abs() < 1e-3, "{media_box:?}");

		let content = String::from_utf8_lossy(page.page_stream().expect("the page has content"));
		assert!(content.contains(color) && content.contains("10 10 m"), "{content}");
		assert!(!content.contains(other_color), "{content}");
	}

	assert!(text.contains("(First)") && text.contains("(Second)"), "{text}");
	assert!(scene.encode_pdf_pages(&[], 96.).is_err());
}

#[test]
fn paths_keep_every_kind_of_segment() {
	let mut path = BezPath::new();
	path.move_to((10., 10.));
	path.line_to((50., 10.));
	path.quad_to((60., 40.), (50., 70.));
	path.curve_to((40., 80.), (20., 80.), (10., 70.));
	path.close_path();

	let text = pdf_text(&encode(|scene| scene.fill_path(Fill::EvenOdd, Affine::IDENTITY, RED, None, &path)));

	// Quadratic segments become the cubic ones a PDF holds, and the even-odd rule fills with `f*`
	assert!(text.contains("10 10 m"), "{text}");
	assert!(text.contains("50 10 l"), "{text}");
	assert!(text.contains(" c\n"), "{text}");
	assert!(text.contains("h\nf*"), "{text}");
	assert!(text.contains("1 0 0 rg"), "{text}");
}

#[test]
fn unbounded_geometry_stays_finite() {
	let mut path = BezPath::new();
	path.move_to((f64::NEG_INFINITY, 20.));
	path.line_to((f64::INFINITY, 20.));
	path.line_to((f64::INFINITY, 40.));
	path.line_to((f64::NEG_INFINITY, 40.));
	path.close_path();

	let text = pdf_text(&encode(|scene| scene.fill_path(Fill::NonZero, Affine::IDENTITY, RED, None, &path)));

	assert!(text.contains("-10000000 20 m"), "{text}");
	assert!(!text.contains("inf") && !text.contains("NaN"), "{text}");
}

#[test]
fn far_geometry_is_clipped_without_bending_its_edges() {
	let (start, end) = clipped_segment(Point::ORIGIN, Point::new(3e7, 1e7)).expect("the segment crosses into the bounds");
	assert_eq!(start, Point::ORIGIN);
	assert_close(end.x, MAX_PDF_COORDINATE);
	assert_close(end.y, MAX_PDF_COORDINATE / 3.);

	// A strip reaching far past the bounds in a rotated layer keeps running through the page along its own direction
	let mut strip = BezPath::new();
	strip.move_to((-1e9, -10.));
	strip.line_to((1e9, -10.));
	strip.line_to((1e9, 10.));
	strip.line_to((-1e9, 10.));
	strip.close_path();
	let placed = Affine::translate((96., 48.)) * Affine::rotate(0.5) * strip;

	let clipped = clipped_to_bounds(&placed, true);
	let along_strip = kurbo::Vec2::from_angle(0.5) * 1000.;
	for point in [Point::new(96., 48.), Point::new(96., 48.) + along_strip, Point::new(96., 48.) - along_strip] {
		assert!(clipped.contains(point), "{point:?} isn't in {clipped:?}");
	}
	assert!(!clipped.contains(Point::new(96., 48.) + along_strip.turn_90() * 0.1));
	assert!(clipped.bounding_box().max_x() <= MAX_PDF_COORDINATE && clipped.bounding_box().min_y() >= -MAX_PDF_COORDINATE);
}

#[test]
fn strokes_keep_their_width_caps_joins_and_dashes() {
	let stroke = Stroke::new(4.).with_caps(Cap::Round).with_join(Join::Bevel).with_dashes(1., [3.]);

	let text = pdf_text(&encode(|scene| scene.stroke(&stroke, Affine::scale(2.), BLUE, None, &Line::new((10., 10.), (40., 20.)))));

	// The stroke is drawn in its own space, so its width scales with its transform (here atop the page's points per pixel) as it does in Vello
	assert!(text.contains("1.5 0 0 -1.5 0 72 cm"), "{text}");
	assert!(text.contains("4 w"), "{text}");
	assert!(text.contains("1 J"), "{text}");
	assert!(text.contains("2 j"), "{text}");
	// An odd-length dash pattern repeats so its dashes and gaps alternate
	assert!(text.contains("[3 3]1 d"), "{text}");
	assert!(text.contains("0 0 1 RG"), "{text}");
}

#[test]
fn zero_width_strokes_draw_nothing() {
	let text = pdf_text(&encode(|scene| scene.stroke(&Stroke::new(0.), Affine::IDENTITY, BLUE, None, &Line::new((10., 10.), (100., 50.)))));

	assert!(!text.contains("\nS\n"), "{text}");
}

#[test]
fn blend_modes_map_to_their_pdf_counterparts() {
	let mixes = [
		Mix::Normal,
		Mix::Multiply,
		Mix::Screen,
		Mix::Overlay,
		Mix::Darken,
		Mix::Lighten,
		Mix::ColorDodge,
		Mix::ColorBurn,
		Mix::HardLight,
		Mix::SoftLight,
		Mix::Difference,
		Mix::Exclusion,
		Mix::Hue,
		Mix::Saturation,
		Mix::Color,
		Mix::Luminosity,
	];
	for mix in mixes {
		assert_eq!(format!("{mix:?}"), format!("{:?}", pdf_blend_mode(mix)));
	}

	let text = pdf_text(&encode(|scene| {
		scene.push_layer(Fill::NonZero, Mix::Multiply, 1., Affine::IDENTITY, &Rect::new(0., 0., 100., 96.));
		scene.fill(Fill::NonZero, Affine::IDENTITY, RED, None, &Rect::new(10., 10., 90., 90.));
		scene.pop_layer();
	}));

	assert!(text.contains("/BM/Multiply"), "{text}");
	// The blended layer is isolated, as is the page beneath it so the blend acts on transparency rather than the paper
	assert!(text.matches("/I true").count() >= 2, "{text}");
}

#[test]
fn translucent_layers_become_opacity_groups() {
	let text = pdf_text(&encode(|scene| {
		scene.push_layer(Fill::NonZero, Mix::Normal, 0.5, Affine::IDENTITY, &Rect::new(0., 0., 100., 96.));
		scene.fill(Fill::NonZero, Affine::IDENTITY, RED, None, &Circle::new((50., 50.), 40.));
		scene.pop_layer();
	}));

	assert!(text.contains("/ca 0.5/CA 0.5"), "{text}");
	assert!(
		text.contains(
			"W
n"
		),
		"{text}"
	);
}

#[test]
fn opaque_normal_layers_only_clip() {
	let text = pdf_text(&encode(|scene| {
		scene.push_layer(Fill::NonZero, Mix::Normal, 1., Affine::IDENTITY, &Rect::new(0., 0., 100., 96.));
		scene.fill(Fill::NonZero, Affine::IDENTITY, RED, None, &Circle::new((50., 50.), 40.));
		scene.pop_layer();
	}));

	assert!(
		text.contains(
			"W
n"
		),
		"{text}"
	);
	assert!(!text.contains("/Group"), "{text}");
}

#[test]
fn porter_duff_layers_become_masks() {
	let draw = |compose| {
		pdf_text(&encode(|scene| {
			let rect = Rect::new(0., 0., 100., 96.);
			scene.push_layer(Fill::NonZero, Mix::Normal, 1., Affine::IDENTITY, &rect);
			scene.fill(Fill::NonZero, Affine::IDENTITY, RED, None, &Circle::new((50., 50.), 40.));
			scene.push_layer(Fill::NonZero, BlendMode::new(Mix::Normal, compose), 1., Affine::IDENTITY, &rect);
			scene.fill(Fill::NonZero, Affine::IDENTITY, BLUE, None, &Rect::new(20., 20., 80., 80.));
			scene.pop_layer();
			scene.pop_layer();
		}))
	};

	// The backdrop becomes the mask of the layer composited onto it
	let source_in = draw(Compose::SrcIn);
	assert!(source_in.contains("/S/Alpha"), "{source_in}");
	assert!(source_in.contains("0 0 1 rg"), "{source_in}");

	// The inverse draws the backdrop in black over white, read as luminosity
	let source_out = draw(Compose::SrcOut);
	assert!(source_out.contains("/S/Luminosity"), "{source_out}");
	assert!(source_out.contains("1 1 1 rg") && source_out.contains("0 0 0 rg"), "{source_out}");
	assert!(!source_out.contains("1 0 0 rg"), "{source_out}");
}

#[test]
fn gradients_become_shadings_with_their_alpha_as_a_mask() {
	let linear = gradient(peniko::LinearGradientPosition::new((0., 0.), (100., 0.)), Extend::Pad, &[(0., RED), (1., BLUE)]);
	let radial = gradient(peniko::RadialGradientPosition::new((50., 50.), 50.), Extend::Pad, &[(0., RED), (1., BLUE.with_alpha(0.))]);

	let text = pdf_text(&encode(|scene| {
		scene.fill(Fill::NonZero, Affine::IDENTITY, &linear, None, &Rect::new(0., 0., 100., 50.));
		scene.fill(Fill::NonZero, Affine::IDENTITY, &radial, None, &Rect::new(0., 50., 100., 96.));
	}));

	assert!(text.contains("/ShadingType 2"), "{text}");
	assert!(text.contains("/ShadingType 3"), "{text}");
	// The radial gradient's fading stop is carried by a luminosity mask of its alpha
	assert!(text.contains("/SMask") && text.contains("/S/Luminosity"), "{text}");
	// Straight sRGB stops need no resampling, so each span is one function
	assert!(text.contains("/C0[1 0 0]/C1[0 0 1]"), "{text}");
}

#[test]
fn repeating_gradients_have_their_repetitions_written_out() {
	let black = AlphaColor::new([0., 0., 0., 1.]);
	let white = AlphaColor::new([1., 1., 1., 1.]);
	let stops = vec![(0., black), (1., white)];

	let repeated = stops_across_range(&stops, Extend::Repeat, 0., 2.).expect("two repetitions fit");
	assert_eq!(repeated, vec![(0., black), (0.5, white), (0.5, black), (1., white)]);

	let reflected = stops_across_range(&stops, Extend::Reflect, 0., 2.).expect("two repetitions fit");
	assert_eq!(reflected, vec![(0., black), (0.5, white), (1., black)]);

	// A range starting partway through a repetition starts at the color there
	let partial = stops_across_range(&stops, Extend::Repeat, 0.5, 1.).expect("half a repetition fits");
	assert_eq!(partial.len(), 2);
	assert_eq!(partial[0].1.components, [0.5, 0.5, 0.5, 1.]);
	assert_eq!(partial[1], (1., white));

	assert!(stops_across_range(&stops, Extend::Repeat, 0., 1e9).is_none());
}

#[test]
fn padded_gradients_ending_on_a_hard_edge_keep_their_outer_color() {
	let white = AlphaColor::new([1., 1., 1., 1.]);
	let blue = AlphaColor::new([0., 0., 1., 1.]);
	let clear = AlphaColor::new([0., 0., 0., 0.]);
	let stops = vec![(0., white), (1., blue), (1., clear)];
	assert!(ends_on_hard_edge(&stops));

	// Across the range alone, the hard edge's inner color ends it
	assert_eq!(stops_across_range(&stops, Extend::Pad, 0., 1.).unwrap(), vec![(0., white), (1., blue)]);

	// Beyond it, the outer color spans the rest
	assert_eq!(stops_across_range(&stops, Extend::Pad, 0., 2.).unwrap(), vec![(0., white), (0.5, blue), (0.5, clear), (1., clear)]);
}

#[test]
fn padded_radial_gradients_ending_on_a_hard_edge_cover_the_shape_they_fill() {
	let clear = peniko::Color::TRANSPARENT;
	let radial = gradient(peniko::RadialGradientPosition::new((0., 0.), 10.), Extend::Pad, &[(0., RED), (1., BLUE), (1., clear)]);

	let text = pdf_text(&encode(|scene| scene.fill(Fill::NonZero, Affine::IDENTITY, &radial, None, &Rect::new(0., 0., 30., 40.))));

	// The circles grow to the farthest corner so the transparency past the edge spans a length of its own
	assert!(text.contains("/Coords[0 0 0 0 0 50]"), "{text}");
}

#[test]
fn repeating_radial_gradients_cover_the_shape_they_fill() {
	let radial = gradient(peniko::RadialGradientPosition::new((0., 0.), 10.), Extend::Repeat, &[(0., RED), (1., BLUE)]);

	let text = pdf_text(&encode(|scene| scene.fill(Fill::NonZero, Affine::IDENTITY, &radial, None, &Rect::new(0., 0., 30., 40.))));

	// The farthest corner is 50 pixels from the center, five repetitions of the ten pixel radius
	assert!(text.contains("/Coords[0 0 0 0 0 50]"), "{text}");
	assert!(text.contains("/ShadingType 3"), "{text}");
}

#[test]
fn non_concentric_repeating_radial_gradients_are_rasterized() {
	let radial = gradient(
		peniko::RadialGradientPosition::new_two_point((10., 10.), 0., (50., 50.), 20.),
		Extend::Reflect,
		&[(0., RED), (1., BLUE)],
	);

	let text = pdf_text(&encode(|scene| scene.fill(Fill::NonZero, Affine::IDENTITY, &radial, None, &Rect::new(0., 0., 100., 96.))));

	assert!(text.contains("/Subtype/Image"), "{text}");
	assert!(!text.contains("/ShadingType"), "{text}");
}

#[test]
fn gradient_positions_follow_each_kind_of_gradient() {
	let linear = GradientKind::Linear(peniko::LinearGradientPosition::new((10., 0.), (20., 0.)));
	assert_close(gradient_position(&linear, Point::new(15., 99.)).unwrap(), 0.5);

	let radial = GradientKind::Radial(peniko::RadialGradientPosition::new((0., 0.), 10.));
	assert_close(gradient_position(&radial, Point::new(3., 4.)).unwrap(), 0.5);

	// Two circles of the same size slide along until one passes through the point
	let sliding = GradientKind::Radial(peniko::RadialGradientPosition::new_two_point((0., 0.), 1., (10., 0.), 1.));
	assert_close(gradient_position(&sliding, Point::new(5., 0.)).unwrap(), 0.6);

	// A quarter turn clockwise, with the page's y-axis pointing down
	let sweep = GradientKind::Sweep(peniko::SweepGradientPosition::new((0., 0.), 0., std::f32::consts::TAU));
	assert!((gradient_position(&sweep, Point::new(0., 1.)).unwrap() - 0.25).abs() < 1e-6);
}

#[test]
fn other_interpolations_are_resampled_into_straight_srgb() {
	let mut premultiplied = gradient(peniko::LinearGradientPosition::new((0., 0.), (1., 0.)), Extend::Pad, &[(0., RED.with_alpha(0.)), (1., BLUE)]);
	premultiplied.interpolation_alpha_space = InterpolationAlphaSpace::Premultiplied;

	let stops = straight_srgb_stops(&premultiplied);
	assert_eq!(stops.len(), RESAMPLED_STOPS_PER_SPAN + 1);

	// The transparent red contributes no color, so the middle is a translucent pure blue
	let (offset, middle) = stops[RESAMPLED_STOPS_PER_SPAN / 2];
	assert_close(offset, 0.5);
	assert!(middle.components[0].abs() < 1e-5 && (middle.components[2] - 1.).abs() < 1e-5, "{middle:?}");
	assert!((middle.components[3] - 0.5).abs() < 1e-5, "{middle:?}");

	// Straight sRGB is already how a PDF interpolates
	let straight = gradient(peniko::LinearGradientPosition::new((0., 0.), (1., 0.)), Extend::Pad, &[(0., RED.with_alpha(0.)), (1., BLUE)]);
	assert_eq!(straight_srgb_stops(&straight).len(), 2);
}

#[test]
fn images_are_embedded_with_their_alpha_channel() {
	let mut image = Image::new(2, 2, Color::WHITE);
	image.data[1] = Color::TRANSPARENT;
	let raster = Raster::new_cpu(image);

	let text = pdf_text(&encode(|scene| scene.draw_raster(raster, peniko::ImageSampler::new(), Affine::scale(10.))));

	assert!(text.contains("/Subtype/Image"), "{text}");
	assert!(text.contains("/Width 2/Height 2"), "{text}");
	assert!(text.contains("/SMask"), "{text}");
	// The image fills its width and height of pixels, scaled by its transform atop the page's points per pixel
	assert!(text.contains("15 0 0 15 0 57 cm"), "{text}");
}

#[test]
fn vector_content_renders_with_its_fill_and_stroke() {
	let (vector, paths) = Vector::from_bezpath(&Rect::new(10., 10., 90., 60.).to_path(0.));
	let red = Graphic::ColorList(List::new_from_element(Color::from_rgbaf32_unchecked(1., 0., 0., 1.)));
	let blue = Graphic::ColorList(List::new_from_element(Color::from_rgbaf32_unchecked(0., 0., 1., 1.)));

	let mut appearance = Appearance::new_single(Coverage::new_fill(), red);
	appearance.replace_or_insert(Coverage::new_stroke(&GraphiteStroke::new(6.)), blue, glam::DAffine2::IDENTITY, CoverPlacement::Above);
	let item = Item::new_from_element(vector).with_attribute(ATTR_PATHS, paths).with_attribute(ATTR_APPEARANCE, appearance);
	let content = List::new_from_item(item);

	let mut scene = PdfSceneBuilder::new(Size::new(100., 100.), Affine::IDENTITY);
	content.render_to_scene(&mut scene, glam::DAffine2::IDENTITY, &RenderParams::default());
	let text = pdf_text(&scene.finish().encode_pdf(96.).expect("the page encodes"));

	assert!(text.contains("1 0 0 rg") && text.contains("\nf\n"), "{text}");
	assert!(text.contains("0 0 1 RG") && text.contains("6 w") && text.contains("\nS\n"), "{text}");
}
