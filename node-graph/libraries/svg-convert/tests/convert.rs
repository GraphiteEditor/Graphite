use core_types::list::{ATTR_APPEARANCE, Item, List};
use core_types::{ATTR_BLEND_MODE, ATTR_EFFECTS, ATTR_GRADIENT_FORM, ATTR_GRADIENT_SPACE, ATTR_GRADIENT_SPREAD, ATTR_OPACITY, ATTR_OPACITY_FILL, ATTR_TRANSFORM, BlendMode, Color};
use glam::{DAffine2, DVec2};
use graphic_types::effects::shadow_offset;
use graphic_types::raster_types::{Image, Raster};
use graphic_types::vector_types::vector::paths_from_list_row_attributes;
use graphic_types::vector_types::vector::style::{StrokeCap, StrokeJoin};
use graphic_types::vector_types::{Gradient, GradientForm, GradientSpace, GradientSpread};
use graphic_types::{Appearance, Cover, Coverage, EffectOperation, Effects, Graphic, Vector};
use kurbo::{BezPath, Point, Shape};
use svg_convert::{Error, Options, convert};

fn converted(content: &str) -> Result<List<Graphic>, Error> {
	convert(&format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">{content}</svg>"#), &Options::default())
}

fn appearance_of(content: &List<Graphic>) -> Option<&Appearance> {
	let Some(Graphic::VectorList(vectors)) = content.element(0) else { return None };
	vectors.attribute(ATTR_APPEARANCE, 0)
}

fn group(content: &List<Graphic>, index: usize) -> Option<&List<Graphic>> {
	content.element(index)?.as_graphic_list()
}

#[test]
fn a_filled_path_becomes_a_vector_whose_fill_coverage_carries_its_color() -> Result<(), Error> {
	let content = converted(r#"<rect width="4" height="4" fill="red" fill-opacity="0.5"/>"#)?;

	let fill = appearance_of(&content).and_then(|appearance| appearance.first_paint_of(Cover::Fill));
	assert_eq!(fill, Some(&Graphic::from(Color::from_gamma_srgb_channels(1., 0., 0., 0.5))));
	Ok(())
}

#[test]
fn a_group_becomes_an_item_carrying_its_transform() -> Result<(), Error> {
	let content = converted(r#"<g transform="translate(3 4)"><rect width="4" height="4"/></g>"#)?;

	assert!(matches!(content.element(0), Some(Graphic::GraphicList(children)) if children.len() == 1));
	assert_eq!(content.attribute::<DAffine2>(ATTR_TRANSFORM, 0), Some(&DAffine2::from_translation(DVec2::new(3., 4.))));
	Ok(())
}

#[test]
fn a_group_item_carries_its_opacity_only_when_below_one() -> Result<(), Error> {
	let half = converted(r#"<g opacity="0.5"><rect width="4" height="4"/></g>"#)?;
	let full = converted(r#"<g opacity="1"><rect width="4" height="4"/></g>"#)?;

	assert_eq!(half.attribute::<f64>(ATTR_OPACITY, 0), Some(&0.5));
	assert_eq!(full.attribute::<f64>(ATTR_OPACITY, 0), None);
	Ok(())
}

#[test]
fn a_group_item_carries_its_blend_mode_unless_it_is_plus_lighter() -> Result<(), Error> {
	let multiply = converted(r#"<g style="mix-blend-mode: multiply"><rect width="4" height="4"/></g>"#)?;
	let plus_lighter = converted(r#"<g style="mix-blend-mode: plus-lighter"><rect width="4" height="4"/></g>"#)?;

	assert_eq!(multiply.attribute::<BlendMode>(ATTR_BLEND_MODE, 0), Some(&BlendMode::Multiply));
	assert_eq!(plus_lighter.attribute::<BlendMode>(ATTR_BLEND_MODE, 0), None);
	Ok(())
}

#[test]
fn text_becomes_a_group_of_glyph_outlines_when_its_font_is_given() -> Result<(), Error> {
	let mut options = Options::default();
	options.fontdb_mut().load_font_data(include_bytes!("../../../nodes/text/src/source-sans-pro-regular.ttf").to_vec());
	let content = convert(
		r#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><text x="2" y="15" font-family="Source Sans Pro" font-size="12">Hi</text></svg>"#,
		&options,
	)?;

	assert!(matches!(content.element(0), Some(Graphic::GraphicList(glyphs)) if matches!(glyphs.element(0), Some(Graphic::VectorList(_)))));
	Ok(())
}

#[test]
fn a_nested_svg_image_becomes_a_group_of_its_content() -> Result<(), Error> {
	let content = converted(
		r#"<image width="4" height="4" href="data:image/svg+xml,%3Csvg%20xmlns=%22http://www.w3.org/2000/svg%22%20width=%224%22%20height=%224%22%3E%3Crect%20width=%224%22%20height=%224%22/%3E%3C/svg%3E"/>"#,
	)?;

	let nested_root = group(&content, 0).and_then(|placement| placement.element(0));
	assert!(matches!(nested_root, Some(Graphic::GraphicList(nested)) if matches!(nested.element(0), Some(Graphic::VectorList(_)))));
	Ok(())
}

#[test]
fn a_raster_image_becomes_a_raster_scaled_to_its_pixel_size() -> Result<(), Error> {
	let png = Raster::new_cpu(Image::new(2, 1, Color::RED)).png_data_url();
	let content = converted(&format!(r#"<image href="{png}"/>"#))?;

	let rasters = match group(&content, 0).and_then(|image| image.element(0)) {
		Some(Graphic::RasterCPUList(rasters)) => Some(rasters),
		_ => None,
	};
	assert_eq!(
		rasters.and_then(|rasters| rasters.attribute::<DAffine2>(ATTR_TRANSFORM, 0)),
		Some(&DAffine2::from_scale(DVec2::new(2., 1.)))
	);
	Ok(())
}

#[test]
fn a_stroked_path_carries_a_stroke_coverage_with_its_paint_width_dashes_cap_and_join() -> Result<(), Error> {
	let content = converted(r#"<path d="M1 1 H7 V7" fill="none" stroke="blue" stroke-opacity="0.5" stroke-width="3" stroke-dasharray="2" stroke-linecap="round" stroke-linejoin="miter-clip"/>"#)?;

	let appearance = appearance_of(&content);
	assert_eq!(
		appearance.and_then(|appearance| appearance.first_paint_of(Cover::Stroke)),
		Some(&Graphic::from(Color::from_gamma_srgb_channels(0., 0., 1., 0.5)))
	);
	assert_eq!(
		appearance
			.and_then(Appearance::first_stroke_params)
			.map(|stroke| (stroke.weight, stroke.dash_lengths, stroke.cap, stroke.join)),
		Some((3., vec![2., 2.], StrokeCap::Round, StrokeJoin::Miter))
	);
	Ok(())
}

#[test]
fn a_stroke_in_a_scaled_group_is_measured_in_the_paths_own_space() -> Result<(), Error> {
	let content = converted(r#"<g transform="scale(2)"><path d="M1 1 H3" stroke="red"/></g>"#)?;

	let stroke = group(&content, 0).and_then(appearance_of).and_then(Appearance::first_stroke_params);
	assert_eq!(stroke.map(|stroke| (stroke.weight, stroke.transform)), Some((1., DAffine2::IDENTITY)));
	Ok(())
}

#[test]
fn paint_order_stroke_puts_the_stroke_coverage_below_the_fill() -> Result<(), Error> {
	let content = converted(r#"<rect width="4" height="4" stroke="red" paint-order="stroke"/>"#)?;

	let stroke_below = appearance_of(&content).map(|appearance| appearance.fill_and_stroke().stroke_below);
	assert_eq!(stroke_below, Some(true));
	Ok(())
}

fn gradient_of(content: &List<Graphic>, cover: Cover) -> Option<&Item<Gradient>> {
	match appearance_of(content)?.first_paint_of(cover)? {
		Graphic::Gradient(gradient) => Some(gradient),
		_ => None,
	}
}

#[test]
fn a_linear_gradient_is_placed_along_its_vector_and_interpolates_in_gamma() -> Result<(), Error> {
	let content = converted(
		r#"<linearGradient id="g" gradientUnits="userSpaceOnUse" x1="1" y1="2" x2="5" y2="2" spreadMethod="reflect"><stop offset="0.25" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient><rect width="8" height="8" fill="url(#g)"/>"#,
	)?;

	let gradient = gradient_of(&content, Cover::Fill);
	assert_eq!(
		gradient.and_then(|gradient| gradient.attribute::<DAffine2>(ATTR_TRANSFORM)),
		Some(&DAffine2::from_cols(DVec2::new(4., 0.), DVec2::new(0., 4.), DVec2::new(1., 2.)))
	);
	assert_eq!(gradient.and_then(|gradient| gradient.attribute::<GradientSpace>(ATTR_GRADIENT_SPACE)), Some(&GradientSpace::RgbGamma));
	assert_eq!(gradient.and_then(|gradient| gradient.attribute::<GradientSpread>(ATTR_GRADIENT_SPREAD)), Some(&GradientSpread::Reflect));
	assert_eq!(gradient.map(|gradient| gradient.element().positions(false)), Some(vec![0.25, 1.]));
	Ok(())
}

#[test]
fn a_zero_length_linear_gradient_paints_the_color_of_its_last_stop() -> Result<(), Error> {
	let content = converted(
		r#"<linearGradient id="g" gradientUnits="userSpaceOnUse" x1="4" y1="4" x2="4" y2="4"><stop stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient><rect width="8" height="8" fill="url(#g)" fill-opacity="0.5"/>"#,
	)?;

	let fill = appearance_of(&content).and_then(|appearance| appearance.first_paint_of(Cover::Fill));
	assert_eq!(fill, Some(&Graphic::from(Color::from_gamma_srgb_channels(0., 0., 1., 0.5))));
	Ok(())
}

#[test]
fn a_bounding_box_radial_gradient_on_a_wide_shape_is_placed_as_an_ellipse() -> Result<(), Error> {
	let content = converted(r#"<radialGradient id="g"><stop stop-color="red"/><stop offset="1" stop-color="blue"/></radialGradient><rect width="8" height="4" fill="url(#g)"/>"#)?;

	let gradient = gradient_of(&content, Cover::Fill);
	assert_eq!(gradient.and_then(|gradient| gradient.attribute::<GradientForm>(ATTR_GRADIENT_FORM)), Some(&GradientForm::Radial));
	assert_eq!(
		gradient.and_then(|gradient| gradient.attribute::<DAffine2>(ATTR_TRANSFORM)),
		Some(&DAffine2::from_cols(DVec2::new(4., 0.), DVec2::new(0., 2.), DVec2::new(4., 2.)))
	);
	Ok(())
}

#[test]
fn a_gradient_stroke_carries_the_stroke_opacity_in_its_stop_alphas() -> Result<(), Error> {
	let content = converted(
		r#"<linearGradient id="g"><stop stop-color="red"/><stop offset="1" stop-color="blue" stop-opacity="0.5"/></linearGradient><rect x="2" y="2" width="4" height="4" fill="none" stroke="url(#g)" stroke-opacity="0.5"/>"#,
	)?;

	let stops = gradient_of(&content, Cover::Stroke).map(|gradient| [gradient.element().0.element(0), gradient.element().0.element(1)]);
	assert_eq!(
		stops,
		Some([Some(&Color::from_gamma_srgb_channels(1., 0., 0., 0.5)), Some(&Color::from_gamma_srgb_channels(0., 0., 1., 0.25))])
	);
	Ok(())
}

#[test]
fn a_clip_path_becomes_a_hidden_clipper_beneath_the_content_it_clips() -> Result<(), Error> {
	let content = converted(r#"<clipPath id="clip"><rect width="4" height="4"/></clipPath><g clip-path="url(#clip)"><rect width="8" height="8"/></g>"#)?;

	let clipped = group(&content, 0);
	assert!(clipped.and_then(|clipped| clipped.element(0)).is_some_and(Graphic::can_reduce_to_clip_path));
	assert_eq!(clipped.and_then(|clipped| clipped.attribute::<f64>(ATTR_OPACITY_FILL, 0)), Some(&0.));
	assert!(clipped.and_then(|clipped| clipped.element(1)).is_some_and(Graphic::had_clip_enabled));
	Ok(())
}

#[test]
fn a_clip_path_transform_places_its_shapes() -> Result<(), Error> {
	let content = converted(r#"<clipPath id="clip" transform="translate(1 2)"><rect width="4" height="4"/></clipPath><g clip-path="url(#clip)"><rect width="8" height="8"/></g>"#)?;

	let shapes = group(&content, 0).and_then(|clipped| clipped.element(0)).and_then(Graphic::as_vector_list);
	assert_eq!(
		shapes.and_then(|shapes| shapes.attribute::<DAffine2>(ATTR_TRANSFORM, 0)),
		Some(&DAffine2::from_translation(DVec2::new(1., 2.)))
	);
	Ok(())
}

#[test]
fn a_clip_path_with_its_own_clip_path_clips_the_content_once_more() -> Result<(), Error> {
	let content = converted(
		r#"<clipPath id="outer"><rect width="6" height="6"/></clipPath><clipPath id="inner" clip-path="url(#outer)"><rect width="4" height="4"/></clipPath><g clip-path="url(#inner)"><rect width="8" height="8"/></g>"#,
	)?;

	let clipped_twice = group(&content, 0);
	assert!(clipped_twice.and_then(|clipped_twice| clipped_twice.element(1)).is_some_and(Graphic::had_clip_enabled));
	let clipped_once = clipped_twice.and_then(|clipped_twice| group(clipped_twice, 1));
	assert!(clipped_once.and_then(|clipped_once| clipped_once.element(1)).is_some_and(Graphic::had_clip_enabled));
	Ok(())
}

fn contours(content: &List<Graphic>, index: usize) -> Vec<BezPath> {
	match content.element(0) {
		Some(Graphic::VectorList(vectors)) => (vectors.element(index).zip(paths_from_list_row_attributes(vectors, index)))
			.map(|(vector, paths)| vector.stroke_bezpath_iter(paths).collect())
			.unwrap_or_default(),
		_ => Vec::new(),
	}
}

#[test]
fn an_even_odd_hole_stays_empty_under_the_nonzero_fill() -> Result<(), Error> {
	let content = converted(r#"<path d="M0 0H8V8H0Z M2 2H6V6H2Z" fill-rule="evenodd"/>"#)?;

	let winding_at = |point: Point| contours(&content, 0).iter().map(|contour| contour.winding(point)).sum::<i32>();
	assert_eq!(winding_at(Point::new(4., 4.)), 0);
	assert_ne!(winding_at(Point::new(1., 1.)), 0);
	Ok(())
}

#[test]
fn an_even_odd_path_filling_the_same_under_nonzero_converts_unchanged() -> Result<(), Error> {
	let ring = r#"d="M0 0H8V8H0Z M2 2V6H6V2Z""#;

	assert_eq!(converted(&format!(r#"<path {ring} fill-rule="evenodd"/>"#))?, converted(&format!("<path {ring}/>"))?);
	Ok(())
}

#[test]
fn a_stroked_even_odd_path_strokes_its_original_outline_above_the_converted_fill() -> Result<(), Error> {
	let rings = r#"d="M0 0H8V8H0Z M2 2H6V6H2Z" stroke="red""#;
	let even_odd = converted(&format!(r#"<path {rings} fill-rule="evenodd"/>"#))?;
	let nonzero = converted(&format!("<path {rings}/>"))?;

	let covers: Vec<Vec<Cover>> = match even_odd.element(0) {
		Some(Graphic::VectorList(vectors)) => vectors
			.iter_attribute_values_or_default::<Appearance>(ATTR_APPEARANCE)
			.map(|appearance| appearance.covers().map(Coverage::cover).collect())
			.collect(),
		_ => Vec::new(),
	};
	assert_eq!(covers, vec![vec![Cover::Fill], vec![Cover::Stroke]]);
	assert_eq!(contours(&even_odd, 1), contours(&nonzero, 0));
	Ok(())
}

#[test]
fn a_luminance_mask_masks_by_the_luminance_of_its_paints_as_alpha() -> Result<(), Error> {
	let content = converted(r##"<mask id="mask"><rect width="8" height="8" fill="#808080"/></mask><g mask="url(#mask)"><rect width="8" height="8"/></g>"##)?;

	let masker = group(&content, 0).and_then(|masked| group(masked, 0));
	let masker_paint = masker.and_then(appearance_of).and_then(|appearance| appearance.first_paint_of(Cover::Fill));
	assert!(matches!(masker_paint, Some(Graphic::Color(color)) if color.element().with_alpha(1.) == Color::BLACK && (color.element().a() - 128. / 255.).abs() < 1e-5));
	Ok(())
}

#[test]
fn a_mask_region_that_cuts_its_content_clips_the_masked_content() -> Result<(), Error> {
	let content = converted(
		r#"<mask id="mask" mask-type="alpha" maskUnits="userSpaceOnUse" x="0" y="0" width="4" height="8"><rect width="8" height="8"/></mask><g mask="url(#mask)"><rect width="8" height="8"/></g>"#,
	)?;

	let within_region = group(&content, 0).and_then(|masked| masked.element(1));
	assert!(within_region.is_some_and(Graphic::had_clip_enabled));
	let clipped = within_region.and_then(Graphic::as_graphic_list);
	assert!(clipped.and_then(|clipped| clipped.element(0)).is_some_and(Graphic::can_reduce_to_clip_path));
	assert!(clipped.and_then(|clipped| clipped.element(1)).is_some_and(Graphic::had_clip_enabled));
	Ok(())
}

#[test]
fn a_gaussian_blur_filter_becomes_a_blur_of_the_geometric_mean_of_its_standard_deviations() -> Result<(), Error> {
	let content = converted(
		r#"<filter id="blur" filterUnits="userSpaceOnUse" x="-4" y="-4" width="16" height="16" color-interpolation-filters="sRGB"><feGaussianBlur in="SourceGraphic" stdDeviation="1 4" color-interpolation-filters="linearRGB" result="effect-1"/><feOffset in="effect-1" result="effect-2"/></filter><g filter="url(#blur)"><rect width="8" height="8"/></g>"#,
	)?;

	let effects = group(&content, 0).and_then(|filtered| filtered.attribute::<Effects>(ATTR_EFFECTS, 0));
	assert_eq!(
		effects,
		Some(&Effects::from_operations([EffectOperation::Blur {
			radius_mask: Graphic::default(),
			radius: 2.,
			box_blur: false,
			gamma: false
		}]))
	);
	Ok(())
}

#[test]
fn a_drop_shadow_falls_by_its_offset_in_its_color() -> Result<(), Error> {
	let content = converted(r#"<g filter="drop-shadow(3px 4px 0 red)"><rect width="4" height="4"/></g>"#)?;

	let shadow = group(&content, 0)
		.and_then(|filtered| filtered.attribute::<Effects>(ATTR_EFFECTS, 0))
		.and_then(|effects| effects.operations().next());
	let (offset, color) = match shadow {
		Some(EffectOperation::DropShadow { color, angle, distance, .. }) => (Some(shadow_offset(angle, distance)), Some(color)),
		_ => (None, None),
	};
	assert!(offset.is_some_and(|offset| offset.abs_diff_eq(DVec2::new(3., 4.), 1e-9)));
	assert_eq!(color, Some(Color::from_gamma_srgb_channels(1., 0., 0., 1.)));
	Ok(())
}

#[test]
fn a_filter_on_a_clipped_group_applies_to_the_content_before_the_clip() -> Result<(), Error> {
	let content = converted(r#"<clipPath id="clip"><rect width="4" height="4"/></clipPath><g clip-path="url(#clip)" filter="blur(1)"><rect width="8" height="8"/></g>"#)?;

	let clipped_content = group(&content, 0).and_then(|clipped| group(clipped, 1));
	assert!(clipped_content.and_then(|filtered| filtered.attribute::<Effects>(ATTR_EFFECTS, 0)).is_some());
	Ok(())
}

fn paint_of(content: &List<Graphic>, cover: Cover) -> Option<&Graphic> {
	appearance_of(content)?.first_paint_of(cover)
}

fn tiles_of(paint: &Graphic) -> Option<&List<Vector>> {
	paint.as_graphic_list()?.element(0)?.as_vector_list()
}

#[test]
fn a_pattern_fill_paints_copies_of_its_tile_over_the_path_at_the_fill_opacity() -> Result<(), Error> {
	let content =
		converted(r#"<pattern id="p" width="10" height="10" patternUnits="userSpaceOnUse"><rect width="5" height="5"/></pattern><rect width="40" height="20" fill="url(#p)" fill-opacity="0.5"/>"#)?;

	let paint = paint_of(&content, Cover::Fill);
	let tiles = paint.and_then(tiles_of);
	assert_eq!(tiles.map(List::len), Some(8));
	assert_eq!(
		tiles.and_then(|tiles| tiles.attribute::<DAffine2>(ATTR_TRANSFORM, 7)),
		Some(&DAffine2::from_translation(DVec2::new(30., 10.)))
	);
	assert_eq!(paint.and_then(Graphic::as_graphic_list).and_then(|paint| paint.attribute::<f64>(ATTR_OPACITY, 0)), Some(&0.5));
	Ok(())
}

#[test]
fn a_pattern_stroke_paints_tiles_over_the_outer_half_of_the_stroke() -> Result<(), Error> {
	let content = converted(
		r#"<pattern id="p" width="10" height="10" patternUnits="userSpaceOnUse"><rect width="5" height="5"/></pattern><rect width="10" height="10" fill="url(#p)" stroke="url(#p)" stroke-width="4"/>"#,
	)?;

	let tile_count = |cover| paint_of(&content, cover).and_then(tiles_of).map(List::len);
	assert_eq!(tile_count(Cover::Fill), Some(1));
	assert_eq!(tile_count(Cover::Stroke), Some(9));
	Ok(())
}

#[test]
fn a_pattern_needing_too_many_tiles_leaves_the_fill_unpainted() -> Result<(), Error> {
	let content = converted(r#"<pattern id="p" width="1" height="1" patternUnits="userSpaceOnUse"><rect width="1" height="1"/></pattern><rect width="1000" height="1000" fill="url(#p)"/>"#)?;

	assert_eq!(paint_of(&content, Cover::Fill), None);
	Ok(())
}

#[test]
fn malformed_input_is_an_error() {
	assert!(convert("<svg", &Options::default()).is_err());
}
