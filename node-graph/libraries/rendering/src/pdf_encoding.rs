//! Writes a recorded [`PdfScene`] out as a vector PDF through krilla.
//!
//! What a PDF can't express gets the closest form it can: Porter-Duff compositing becomes masking, repeating gradients have
//! their repetitions written out, and gradients beyond that are rasterized and clipped to their shape.

use crate::pdf_scene_builder::{PdfDrawing, PdfGroupKind, PdfImage, PdfPaint, PdfScene};
use krilla::Document;
use krilla::blend::BlendMode as PdfBlendMode;
use krilla::color::rgb;
use krilla::geom::{Path as PdfPath, PathBuilder, Size as PdfSize, Transform as PdfTransform};
use krilla::image::{BitsPerComponent, CustomImage, Image as PdfEmbeddedImage, ImageColorspace};
use krilla::mask::{Mask, MaskType};
use krilla::num::NormalizedF32;
use krilla::page::PageSettings;
use krilla::paint::{Fill as PdfFill, FillRule, LineCap, LineJoin, LinearGradient, Paint, RadialGradient, SpreadMethod, Stop, Stroke as PdfStroke, StrokeDash, SweepGradient};
use krilla::surface::Surface;
use kurbo::{Affine, BezPath, Cap, Join, PathEl, Point, Rect, Shape, Stroke, StrokeOpts};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use vello_gpu::peniko::color::{AlphaColor, ColorSpaceTag, Srgb};
use vello_gpu::peniko::{self, Compose, Extend, Fill, GradientKind, ImageAlphaType, ImageFormat, ImageQuality, InterpolationAlphaSpace, Mix};

/// PDF measures lengths in points, each a 72nd of an inch.
const POINTS_PER_INCH: f64 = 72.;
/// The resolution content is rasterized at where a PDF can't express it.
const RASTERIZATION_PIXELS_PER_INCH: f64 = 300.;
/// The largest width or height of content rasterized for a PDF, in pixels.
const MAX_RASTERIZATION_SIZE: f64 = 4096.;
/// The most stops a repeating gradient's repetitions are written out with, beyond which it's rasterized.
const MAX_REPEATED_GRADIENT_STOPS: usize = 4096;
/// The most stops krilla writes a sweep gradient with before it starts leaving stops out.
const MAX_SWEEP_GRADIENT_STOPS: usize = 97;
/// The stops that stand in for each span of a gradient interpolated other than in a PDF's straight-alpha sRGB.
const RESAMPLED_STOPS_PER_SPAN: usize = 16;
/// Bounds every coordinate written to a PDF, far past the page, since unbounded geometry has to end somewhere.
const MAX_PDF_COORDINATE: f64 = 1e7;
/// How closely the polylines standing in for the curves of subpaths clipped to [`MAX_PDF_COORDINATE`] follow them, in the page's pixels.
const POLYLINE_TOLERANCE: f64 = 0.01;
/// Matches the threshold below which Vello treats a gradient's geometry as degenerate.
const DEGENERATE_GRADIENT_THRESHOLD: f64 = 1e-6;

/// A gradient's colors at positions along it, in straight-alpha sRGB.
type ColorStops = Vec<(f64, AlphaColor<Srgb>)>;

/// An image embedded in a PDF, with its width and height in pixels.
type SizedImage = (PdfEmbeddedImage, (u32, u32));

impl PdfScene {
	/// Encodes the page as a PDF document, sized so `pixels_per_inch` of its pixels span each inch of paper.
	pub fn encode_pdf(&self, pixels_per_inch: f64) -> Result<Vec<u8>, String> {
		let points_per_pixel = POINTS_PER_INCH / pixels_per_inch;
		if !points_per_pixel.is_finite() || points_per_pixel <= 0. {
			return Err(format!("A PDF can't be sized at {pixels_per_inch} pixels per inch"));
		}

		let width = (self.size.width * points_per_pixel) as f32;
		let height = (self.size.height * points_per_pixel) as f32;
		let Some(page_settings) = PageSettings::from_wh(width, height) else {
			return Err("The PDF page has no area".to_string());
		};

		let mut document = Document::new();
		let mut page = document.start_page_with(page_settings);
		let mut surface = page.surface();

		// The drawings are in the page's pixels, which this scales into points
		surface.push_transform(&PdfTransform::from_scale(points_per_pixel as f32, points_per_pixel as f32));

		// Blending at the top acts on the transparent page, as it does in the other export formats, rather than on the paper beneath it
		let isolate = blends_with_backdrop(&self.drawings);
		if isolate {
			surface.push_isolated();
		}

		let mut encoder = PdfEncoder::new(self, points_per_pixel);
		encoder.draw_drawings(&mut surface, &self.drawings);

		if isolate {
			surface.pop();
		}
		surface.pop();
		surface.finish();
		page.finish();

		document.finish().map_err(|error| format!("Failed to write the PDF: {error}"))
	}
}

/// What a fill or stroke paints with once it's in a form the PDF can hold.
enum ResolvedPaint {
	Vector {
		paint: Paint,
		opacity: NormalizedF32,
	},
	/// Pixels drawn within the shape in place of the paint, placed by the transform from their pixels into the shape's space.
	Pixels {
		image: PdfEmbeddedImage,
		size: (u32, u32),
		transform: Affine,
	},
}

struct PdfEncoder<'a> {
	scene: &'a PdfScene,
	points_per_pixel: f64,
	/// Each image once embedded, by its index, whether it's interpolated, and whether it's painted black.
	embedded_images: HashMap<(usize, bool, bool), Option<SizedImage>>,
	/// Whether paints draw black at their own alpha, which turns content into an inverted alpha mask when drawn over white as a luminosity mask.
	paint_black: bool,
	warnings: HashSet<&'static str>,
}

impl<'a> PdfEncoder<'a> {
	fn new(scene: &'a PdfScene, points_per_pixel: f64) -> Self {
		Self {
			scene,
			points_per_pixel,
			embedded_images: HashMap::new(),
			paint_black: false,
			warnings: HashSet::new(),
		}
	}

	fn warn_once(&mut self, message: &'static str) {
		if self.warnings.insert(message) {
			log::warn!("{message}");
		}
	}

	/// The part of the page `bounds` covers, or `None` if it's off the page.
	fn visible_region(&self, bounds: Rect) -> Option<Rect> {
		let region = bounds.intersect(Rect::from_origin_size((0., 0.), self.scene.size));
		(region.width() > 0. && region.height() > 0.).then_some(region)
	}

	fn draw_drawings(&mut self, surface: &mut Surface, drawings: &[PdfDrawing]) {
		// A layer composited by a Porter-Duff operator acts on everything before it in its group, so the last one splits that off as its backdrop
		let Some(index) = drawings.iter().rposition(composites_with_backdrop) else {
			for drawing in drawings {
				self.draw_drawing(surface, drawing);
			}
			return;
		};

		let (backdrop, [layer, rest @ ..]) = drawings.split_at(index) else { return };
		self.draw_composited_layer(surface, backdrop, layer);
		for drawing in rest {
			self.draw_drawing(surface, drawing);
		}
	}

	fn draw_drawing(&mut self, surface: &mut Surface, drawing: &PdfDrawing) {
		match drawing {
			PdfDrawing::Fill { path, fill_rule, paint } => self.fill(surface, path, *fill_rule, paint),
			PdfDrawing::Stroke { path, transform, stroke, paint } => self.stroke(surface, path, *transform, stroke, paint),
			PdfDrawing::Image { image, quality, alpha, transform } => self.draw_image(surface, *image, *quality != ImageQuality::Low, *alpha, *transform),
			PdfDrawing::Group {
				kind: PdfGroupKind::Layer { blend_mode, alpha, clip },
				drawings,
			} => self.draw_layer(surface, blend_mode.mix, *alpha, clip.as_ref(), drawings),
			PdfDrawing::Group {
				kind: PdfGroupKind::Clip { path, fill_rule },
				drawings,
			} => {
				// An empty clip leaves nothing within it visible
				let Some(path) = pdf_path(&clipped_to_bounds(path, true)) else { return };

				surface.push_clip_path(&path, &pdf_fill_rule(*fill_rule));
				self.draw_drawings(surface, drawings);
				surface.pop();
			}
		}
	}

	/// Draws a layer composited onto its backdrop by a Porter-Duff operator, which a PDF can only express by one masking the other.
	/// The layer's clip is taken to cover its backdrop, as it does for every layer the renderer composites this way.
	fn draw_composited_layer(&mut self, surface: &mut Surface, backdrop: &[PdfDrawing], layer: &PdfDrawing) {
		let PdfDrawing::Group {
			kind: PdfGroupKind::Layer { blend_mode, alpha, clip },
			drawings,
		} = layer
		else {
			return;
		};
		let draw_layer = |encoder: &mut Self, surface: &mut Surface| encoder.draw_layer(surface, blend_mode.mix, *alpha, clip.as_ref(), drawings);
		let draw_backdrop = |encoder: &mut Self, surface: &mut Surface| encoder.draw_drawings(surface, backdrop);

		match blend_mode.compose {
			Compose::SrcIn | Compose::SrcOut => {
				let mask = self.mask(surface, blend_mode.compose == Compose::SrcOut, draw_backdrop);
				surface.push_mask(mask);
				draw_layer(self, surface);
				surface.pop();
			}
			Compose::DestIn | Compose::DestOut => {
				let mask = self.mask(surface, blend_mode.compose == Compose::DestOut, draw_layer);
				surface.push_mask(mask);
				draw_backdrop(self, surface);
				surface.pop();
			}
			_ => {
				self.warn_once("A layer's compositing operator isn't supported in PDF export, so it's composited normally");
				draw_backdrop(self, surface);
				draw_layer(self, surface);
			}
		}
	}

	/// A mask of the alpha of what `draw` draws, or of one minus that alpha when `inverted`.
	fn mask(&mut self, surface: &mut Surface, inverted: bool, draw: impl FnOnce(&mut Self, &mut Surface)) -> Mask {
		let mut stream_builder = surface.stream_builder();
		let mut mask_surface = stream_builder.surface();

		if inverted {
			// Content in black over white leaves a luminosity of one minus the content's alpha
			let page = Rect::from_origin_size((0., 0.), self.scene.size).to_path(0.);
			if let Some(page) = pdf_path(&page) {
				mask_surface.set_stroke(None);
				mask_surface.set_fill(Some(PdfFill {
					paint: rgb::Color::white().into(),
					opacity: NormalizedF32::ONE,
					rule: FillRule::NonZero,
				}));
				mask_surface.draw_path(&page);
			}

			let paint_black = std::mem::replace(&mut self.paint_black, true);
			draw(self, &mut mask_surface);
			self.paint_black = paint_black;
		} else {
			draw(self, &mut mask_surface);
		}

		mask_surface.finish();
		let mask_type = if inverted { MaskType::Luminosity } else { MaskType::Alpha };
		Mask::new(stream_builder.finish(), mask_type)
	}

	fn draw_layer(&mut self, surface: &mut Surface, mix: Mix, alpha: f32, clip: Option<&(BezPath, Fill)>, drawings: &[PdfDrawing]) {
		if alpha.is_nan() || alpha <= 0. {
			return;
		}

		let clip = match clip {
			Some((path, fill_rule)) => match pdf_path(&clipped_to_bounds(path, true)) {
				Some(path) => Some((path, pdf_fill_rule(*fill_rule))),
				// An empty clip leaves nothing within it visible
				None => return,
			},
			None => None,
		};
		if let Some((path, fill_rule)) = &clip {
			surface.push_clip_path(path, fill_rule);
		}

		// Only alpha matters in content painted black for a mask, and modes like screen would let the white beneath show through
		let blend_mode = if self.paint_black { PdfBlendMode::Normal } else { pdf_blend_mode(mix) };
		if blend_mode != PdfBlendMode::Normal {
			surface.push_blend_mode(blend_mode);
		}

		// A translucent layer is grouped by its opacity, and a blended one by an isolated group so it blends with its backdrop as a whole
		let translucent = alpha < 1.;
		let isolated = !translucent && (blend_mode != PdfBlendMode::Normal || blends_with_backdrop(drawings));
		if translucent {
			surface.push_opacity(normalized(alpha));
		} else if isolated {
			surface.push_isolated();
		}

		self.draw_drawings(surface, drawings);

		if translucent || isolated {
			surface.pop();
		}
		if blend_mode != PdfBlendMode::Normal {
			surface.pop();
		}
		if clip.is_some() {
			surface.pop();
		}
	}

	fn fill(&mut self, surface: &mut Surface, path: &BezPath, fill_rule: Fill, paint: &PdfPaint) {
		let path = clipped_to_bounds(path, true);
		let Some(shape) = pdf_path(&path) else { return };
		let Some(region) = self.visible_region(path.bounding_box()) else { return };
		let Some(paint) = self.resolve_paint(paint, region, Affine::IDENTITY) else { return };
		let fill_rule = pdf_fill_rule(fill_rule);

		match paint {
			ResolvedPaint::Vector { paint, opacity } => {
				surface.set_stroke(None);
				surface.set_fill(Some(PdfFill { paint, opacity, rule: fill_rule }));
				surface.draw_path(&shape);
				surface.set_fill(None);
			}
			ResolvedPaint::Pixels { image, size, transform } => {
				surface.push_clip_path(&shape, &fill_rule);
				draw_pixels(surface, image, size, transform);
				surface.pop();
			}
		}
	}

	/// Strokes a path given in the space that `transform` places onto the page.
	fn stroke(&mut self, surface: &mut Surface, path: &BezPath, transform: Affine, stroke: &Stroke, paint: &PdfPaint) {
		// Vello strokes nothing for a zero width, whereas a PDF strokes its thinnest line
		if stroke.width.is_nan() || stroke.width <= 0. || !(stroke.width as f32).is_finite() || !is_placeable(transform) {
			return;
		}

		// Move the path within its own space to put the page's origin at its origin, so the f32 coordinates a PDF holds stay precise near the page
		let [a, b, c, d, e, f] = transform.as_coeffs();
		let transform = Affine::new([a, b, c, d, 0., 0.]);
		let to_page_origin = Affine::translate((transform.inverse() * Point::new(e, f)).to_vec2());
		let paint = match paint {
			PdfPaint::Solid(_) => Cow::Borrowed(paint),
			PdfPaint::Gradient { gradient, transform } => Cow::Owned(PdfPaint::Gradient {
				gradient: gradient.clone(),
				transform: to_page_origin * *transform,
			}),
			PdfPaint::Image { image, sampler, transform } => Cow::Owned(PdfPaint::Image {
				image: *image,
				sampler: *sampler,
				transform: to_page_origin * *transform,
			}),
		};
		let mut path = to_page_origin * path;
		if let Cow::Owned(clipped) = clipped_to_bounds(&(transform * &path), false) {
			path = transform.inverse() * clipped;
		}
		let Some(shape) = pdf_path(&path) else { return };

		// The stroke's reach past its path, generously covering its joins and caps
		let reach = stroke.width / 2. * stroke.miter_limit.max(std::f64::consts::SQRT_2);
		let Some(region) = self.visible_region(transform.transform_rect_bbox(path.bounding_box().inflate(reach, reach))) else {
			return;
		};
		let Some(paint) = self.resolve_paint(&paint, region, transform) else { return };

		surface.push_transform(&pdf_transform(transform));
		match paint {
			ResolvedPaint::Vector { paint, opacity } => {
				surface.set_fill(None);
				surface.set_stroke(Some(pdf_stroke(stroke, paint, opacity)));
				surface.draw_path(&shape);
				surface.set_stroke(None);
			}
			ResolvedPaint::Pixels {
				image,
				size,
				transform: image_transform,
			} => {
				// Pixels in place of a stroke's paint are clipped to the stroke's outline, expanded finely enough to look smooth on paper
				let largest_scale = [a, b, c, d].into_iter().fold(0., |largest: f64, coefficient| largest.max(coefficient.abs()));
				let tolerance = 0.01 / largest_scale.max(1e-9);
				let outline = kurbo::stroke(path.iter(), stroke, &StrokeOpts::default(), tolerance);
				if let Some(outline) = pdf_path(&outline) {
					surface.push_clip_path(&outline, &FillRule::NonZero);
					draw_pixels(surface, image, size, image_transform);
					surface.pop();
				}
			}
		}
		surface.pop();
	}

	fn draw_image(&mut self, surface: &mut Surface, image: usize, interpolate: bool, alpha: f32, transform: Affine) {
		if alpha.is_nan() || alpha <= 0. || !is_placeable(transform) {
			return;
		}
		// An image off the page needn't be embedded, and one reaching past what a PDF's numbers hold can't be
		let (width, height) = match self.scene.images.get(image) {
			Some(PdfImage::Pixels(data)) => (data.width, data.height),
			Some(PdfImage::Raster(raster)) => (raster.width, raster.height),
			Some(PdfImage::Texture(texture)) => (texture.width(), texture.height()),
			None => return,
		};
		let bounds = transform.transform_rect_bbox(Rect::new(0., 0., width as f64, height as f64));
		let fits_in_pdf = [bounds.x0, bounds.y0, bounds.x1, bounds.y1].iter().all(|coordinate| (*coordinate as f32).is_finite());
		if !fits_in_pdf || self.visible_region(bounds).is_none() {
			return;
		}

		let Some((image, size)) = self.embedded_image(image, interpolate) else { return };

		if alpha < 1. {
			surface.push_opacity(normalized(alpha));
		}
		draw_pixels(surface, image, size, transform);
		if alpha < 1. {
			surface.pop();
		}
	}

	/// The paint in a form the PDF can hold, for a shape covering `region` of the page in a space that `shape_to_page` places onto it.
	fn resolve_paint(&mut self, paint: &PdfPaint, region: Rect, shape_to_page: Affine) -> Option<ResolvedPaint> {
		match paint {
			PdfPaint::Solid(color) => {
				let (color, opacity) = self.pdf_color(*color);
				Some(ResolvedPaint::Vector { paint: color.into(), opacity })
			}
			PdfPaint::Gradient { gradient, transform } => self.gradient_paint(gradient, *transform, region, shape_to_page),
			PdfPaint::Image { image, sampler, transform } => {
				if sampler.x_extend != Extend::Pad || sampler.y_extend != Extend::Pad {
					self.warn_once("An image paint's repetition isn't supported in PDF export, so it's drawn once");
				}
				if !is_placeable(*transform) {
					return None;
				}

				let (image, size) = self.embedded_image(*image, sampler.quality != ImageQuality::Low)?;
				Some(ResolvedPaint::Pixels { image, size, transform: *transform })
			}
		}
	}

	/// A gradient placed by `transform` into the space of a shape covering `region` of the page, in the form a PDF can hold.
	fn gradient_paint(&mut self, gradient: &peniko::Gradient, transform: Affine, region: Rect, shape_to_page: Affine) -> Option<ResolvedPaint> {
		// Vello leaves out a gradient it can't place
		if !is_placeable(transform) {
			return None;
		}

		let mut stops = straight_srgb_stops(gradient);
		if self.paint_black {
			for (_, color) in &mut stops {
				*color = AlphaColor::new([0., 0., 0., color.components[3]]);
			}
		}

		// Like Vello, a degenerate gradient paints its first color
		let first_color = stops.first()?.1;
		if stops.len() == 1 || gradient_is_degenerate(gradient) {
			let (color, opacity) = pdf_color_components(first_color);
			return Some(ResolvedPaint::Vector { paint: color.into(), opacity });
		}

		let gradient_to_page = shape_to_page * transform;
		let region_in_gradient = gradient_to_page.inverse() * region.to_path(0.);
		let region_corners: Vec<Point> = region_in_gradient.elements().iter().filter_map(|element| element.end_point()).collect();
		let pdf_transform = pdf_transform(transform);

		let spread_method = match gradient.extend {
			Extend::Pad => SpreadMethod::Pad,
			Extend::Repeat => SpreadMethod::Repeat,
			Extend::Reflect => SpreadMethod::Reflect,
		};

		let vector_paint = |paint: Paint| Some(ResolvedPaint::Vector { paint, opacity: NormalizedF32::ONE });
		let rasterized = |encoder: &mut Self, stops: &ColorStops| encoder.rasterized_gradient(gradient, stops, gradient_to_page, region, shape_to_page);

		// A PDF only extends a gradient's end colors, so a repeating one has its repetitions across the region written out.
		// So does a padded one ending on a hard edge, since some viewers skip an outer color that spans no length.
		let written_out = gradient.extend != Extend::Pad || ends_on_hard_edge(&stops);
		let whole_range = |range_start: f64, range_end: f64| match gradient.extend {
			Extend::Pad => (range_start.min(0.), range_end.max(1.)),
			Extend::Repeat | Extend::Reflect => (range_start.floor(), range_end.ceil()),
		};

		match gradient.kind {
			GradientKind::Linear(position) => {
				let (start, end) = (position.start, position.end);
				if !written_out {
					return vector_paint(pdf_linear_gradient(start, end, pdf_transform, &stops));
				}

				let direction = end - start;
				let position_along = |point: &Point| (*point - start).dot(direction) / direction.hypot2();
				let (range_start, range_end) = range_of(region_corners.iter().map(position_along));
				let (range_start, range_end) = whole_range(range_start, range_end);
				match stops_across_range(&stops, gradient.extend, range_start, range_end) {
					Some(spread) => vector_paint(pdf_linear_gradient(start + direction * range_start, start + direction * range_end, pdf_transform, &spread)),
					None => rasterized(self, &stops),
				}
			}
			GradientKind::Radial(position) => {
				let center = position.start_center;
				let (start_radius, end_radius) = (position.start_radius as f64, position.end_radius as f64);
				let radius_growth = end_radius - start_radius;
				let concentric = (position.end_center - center).hypot() <= DEGENERATE_GRADIENT_THRESHOLD;
				if !written_out || (gradient.extend == Extend::Pad && !concentric) {
					return vector_paint(pdf_radial_gradient(position, pdf_transform, &stops));
				}

				// Concentric circles can cover the region by growing the outer one, which a pair of circles moving apart can't
				if !concentric || radius_growth.abs() <= DEGENERATE_GRADIENT_THRESHOLD {
					return rasterized(self, &stops);
				}

				let farthest = region_corners.iter().fold(0., |farthest: f64, corner| farthest.max((*corner - center).hypot()));
				let position_at = |distance: f64| (distance - start_radius) / radius_growth;
				let (range_start, range_end) = range_of([position_at(0.), position_at(farthest)].into_iter());
				let (mut range_start, mut range_end) = whole_range(range_start, range_end);
				// A circle can't shrink past zero radius
				let zero_radius = -start_radius / radius_growth;
				if radius_growth > 0. {
					range_start = range_start.max(zero_radius);
				} else {
					range_end = range_end.min(zero_radius);
				}

				match stops_across_range(&stops, gradient.extend, range_start, range_end) {
					Some(spread) => {
						let position = peniko::RadialGradientPosition {
							start_center: center,
							start_radius: (start_radius + radius_growth * range_start) as f32,
							end_center: center,
							end_radius: (start_radius + radius_growth * range_end) as f32,
						};
						vector_paint(pdf_radial_gradient(position, pdf_transform, &spread))
					}
					None => rasterized(self, &stops),
				}
			}
			GradientKind::Sweep(position) => {
				if stops.len() > MAX_SWEEP_GRADIENT_STOPS {
					return rasterized(self, &stops);
				}

				vector_paint(
					SweepGradient {
						cx: position.center.x as f32,
						cy: position.center.y as f32,
						start_angle: position.start_angle.to_degrees(),
						end_angle: position.end_angle.to_degrees(),
						transform: pdf_transform,
						spread_method,
						stops: pdf_stops(&stops),
						anti_alias: false,
					}
					.into(),
				)
			}
		}
	}

	/// Pixels showing a gradient across `region` of the page, for one a PDF can't express, placed into the space of the shape it paints.
	fn rasterized_gradient(&mut self, gradient: &peniko::Gradient, stops: &ColorStops, gradient_to_page: Affine, region: Rect, shape_to_page: Affine) -> Option<ResolvedPaint> {
		self.warn_once("A gradient a PDF can't express is rasterized in PDF export");

		let pixels_per_page_pixel = self.points_per_pixel * RASTERIZATION_PIXELS_PER_INCH / POINTS_PER_INCH;
		let pixel_count = |length: f64| (length * pixels_per_page_pixel).ceil().clamp(1., MAX_RASTERIZATION_SIZE) as u32;
		let (width, height) = (pixel_count(region.width()), pixel_count(region.height()));

		let pixels_to_page = Affine::translate(region.origin().to_vec2()) * Affine::scale_non_uniform(region.width() / width as f64, region.height() / height as f64);
		let pixels_to_gradient = gradient_to_page.inverse() * pixels_to_page;

		// O(width * height * log(stops))
		let mut rgba = vec![0; width as usize * height as usize * 4];
		for (index, pixel) in rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
			let (x, y) = ((index % width as usize) as f64 + 0.5, (index / width as usize) as f64 + 0.5);
			let Some(position) = gradient_position(&gradient.kind, pixels_to_gradient * Point::new(x, y)) else {
				continue;
			};

			let position = match gradient.extend {
				Extend::Pad => position.clamp(0., 1.),
				Extend::Repeat => position - position.floor(),
				Extend::Reflect => 1. - (position.rem_euclid(2.) - 1.).abs(),
			};
			*pixel = sample_stops(stops, position, false).to_rgba8().to_u8_array();
		}

		let image = pdf_embedded_image(rgba, width, height, true)?;
		let transform = shape_to_page.inverse() * pixels_to_page;
		Some(ResolvedPaint::Pixels {
			image,
			size: (width, height),
			transform,
		})
	}

	/// The image at `index`, embedded the first time it's drawn, or `None` if it has no pixels to embed.
	fn embedded_image(&mut self, index: usize, interpolate: bool) -> Option<SizedImage> {
		let key = (index, interpolate, self.paint_black);
		if let Some(embedded) = self.embedded_images.get(&key) {
			return embedded.clone();
		}

		let pixels = match self.scene.images.get(index)? {
			PdfImage::Pixels(data) => straight_rgba8(data),
			PdfImage::Raster(raster) => Some(raster.to_flat_u8()),
			PdfImage::Texture(_) => {
				self.warn_once("A GPU texture wasn't read back before PDF export, so the PDF leaves it out");
				None
			}
		};
		let embedded = pixels.and_then(|(mut rgba, width, height)| {
			if self.paint_black {
				for pixel in rgba.as_chunks_mut::<4>().0 {
					pixel[..3].fill(0);
				}
			}
			pdf_embedded_image(rgba, width, height, interpolate).map(|image| (image, (width, height)))
		});

		self.embedded_images.insert(key, embedded.clone());
		embedded
	}

	fn pdf_color(&self, color: peniko::Color) -> (rgb::Color, NormalizedF32) {
		if self.paint_black {
			return (rgb::Color::black(), normalized(color.components[3]));
		}
		pdf_color_components(color)
	}
}

/// Whether a drawing is a layer composited onto what's beneath it by a Porter-Duff operator other than the usual source-over.
fn composites_with_backdrop(drawing: &PdfDrawing) -> bool {
	matches!(drawing, PdfDrawing::Group { kind: PdfGroupKind::Layer { blend_mode, .. }, .. } if blend_mode.compose != Compose::SrcOver)
}

/// Whether drawings blend with what's beneath them other than normally, so a group of them must be isolated to blend only among themselves.
fn blends_with_backdrop(drawings: &[PdfDrawing]) -> bool {
	drawings.iter().any(|drawing| match drawing {
		PdfDrawing::Group {
			kind: PdfGroupKind::Layer { blend_mode, .. },
			..
		} => blend_mode.mix != Mix::Normal,
		PdfDrawing::Group {
			kind: PdfGroupKind::Clip { .. },
			drawings,
		} => blends_with_backdrop(drawings),
		_ => false,
	})
}

fn draw_pixels(surface: &mut Surface, image: PdfEmbeddedImage, (width, height): (u32, u32), transform: Affine) {
	let Some(size) = PdfSize::from_wh(width as f32, height as f32) else { return };

	surface.push_transform(&pdf_transform(transform));
	surface.draw_image(image, size);
	surface.pop();
}

/// Whether a transform can place content in a PDF, being invertible in the f32 numbers a PDF holds, as krilla relies on.
fn is_placeable(transform: Affine) -> bool {
	let determinant = transform.determinant();
	let invertible = determinant != 0. && determinant.recip().is_finite();
	invertible && transform.as_coeffs().iter().all(|coefficient| (*coefficient as f32).is_finite()) && pdf_transform(transform).invert().is_some()
}

fn normalized(value: f32) -> NormalizedF32 {
	NormalizedF32::new(value.clamp(0., 1.)).unwrap_or(NormalizedF32::ZERO)
}

fn pdf_transform(transform: Affine) -> PdfTransform {
	let [a, b, c, d, e, f] = transform.as_coeffs().map(|coefficient| coefficient as f32);
	PdfTransform::from_row(a, b, c, d, e, f)
}

/// The path in a PDF's numbers, or `None` if it encloses nothing it could draw.
fn pdf_path(path: &BezPath) -> Option<PdfPath> {
	let point = |point: Point| {
		(
			point.x.clamp(-MAX_PDF_COORDINATE, MAX_PDF_COORDINATE) as f32,
			point.y.clamp(-MAX_PDF_COORDINATE, MAX_PDF_COORDINATE) as f32,
		)
	};

	let mut builder = PathBuilder::new();
	for element in path.elements() {
		match *element {
			PathEl::MoveTo(p) => {
				let (x, y) = point(p);
				builder.move_to(x, y);
			}
			PathEl::LineTo(p) => {
				let (x, y) = point(p);
				builder.line_to(x, y);
			}
			PathEl::QuadTo(p1, p2) => {
				let ((x1, y1), (x, y)) = (point(p1), point(p2));
				builder.quad_to(x1, y1, x, y);
			}
			PathEl::CurveTo(p1, p2, p3) => {
				let ((x1, y1), (x2, y2), (x, y)) = (point(p1), point(p2), point(p3));
				builder.cubic_to(x1, y1, x2, y2, x, y);
			}
			PathEl::ClosePath => builder.close(),
		}
	}
	builder.finish()
}

/// The axes and sides of the half-planes that bound the coordinates written to a PDF, by the axis index and the sign of its bound.
const BOUNDING_HALF_PLANES: [(usize, f64); 4] = [(0, 1.), (0, -1.), (1, 1.), (1, -1.)];

/// The path with each subpath reaching past [`MAX_PDF_COORDINATE`] clipped to that bound, keeping the direction of every edge it cuts.
/// The segments of those subpaths reaching past the bound become lines and the rest become fine polylines, which reshapes nothing near the page.
/// A `closed` path is clipped as the area it fills, and an open one as the lines it strokes.
fn clipped_to_bounds(path: &BezPath, closed: bool) -> Cow<'_, BezPath> {
	let within_bounds = |point: &Point| point.x.abs() <= MAX_PDF_COORDINATE && point.y.abs() <= MAX_PDF_COORDINATE;
	let element_points = |element: &PathEl| match *element {
		PathEl::MoveTo(p) | PathEl::LineTo(p) => vec![p],
		PathEl::QuadTo(p1, p2) => vec![p1, p2],
		PathEl::CurveTo(p1, p2, p3) => vec![p1, p2, p3],
		PathEl::ClosePath => Vec::new(),
	};
	if path.elements().iter().all(|element| element_points(element).iter().all(within_bounds)) {
		return Cow::Borrowed(path);
	}

	let mut result = BezPath::new();
	for subpath in path.elements().chunk_by(|_, next| !matches!(next, PathEl::MoveTo(_))) {
		if subpath.iter().all(|element| element_points(element).iter().all(within_bounds)) {
			result.extend(subpath.iter().copied());
			continue;
		}

		// The subpath as a polyline, with the segments reaching past the bound kept straight
		let mut vertices = Vec::new();
		let mut current = Point::ORIGIN;
		for element in subpath {
			let segment = match *element {
				PathEl::MoveTo(p) => {
					current = p;
					vertices.push(p);
					continue;
				}
				PathEl::ClosePath => continue,
				PathEl::LineTo(p) => kurbo::PathSeg::Line(kurbo::Line::new(current, p)),
				PathEl::QuadTo(p1, p2) => kurbo::PathSeg::Quad(kurbo::QuadBez::new(current, p1, p2)),
				PathEl::CurveTo(p1, p2, p3) => kurbo::PathSeg::Cubic(kurbo::CubicBez::new(current, p1, p2, p3)),
			};
			let start = current;
			current = element.end_point().unwrap_or(current);

			if within_bounds(&start) && element_points(element).iter().all(within_bounds) {
				kurbo::flatten(BezPath::from_path_segments(std::iter::once(segment)), POLYLINE_TOLERANCE, |element| {
					if let PathEl::LineTo(p) = element {
						vertices.push(p);
					}
				});
			} else {
				vertices.push(current);
			}
		}

		if closed {
			let clipped = clipped_polygon(vertices);
			if let [first, rest @ ..] = clipped.as_slice() {
				result.move_to(*first);
				rest.iter().for_each(|&point| result.line_to(point));
				result.close_path();
			}
		} else {
			if matches!(subpath.last(), Some(PathEl::ClosePath))
				&& let Some(&first) = vertices.first()
			{
				vertices.push(first);
			}
			for pair in vertices.windows(2) {
				let Some((start, end)) = clipped_segment(pair[0], pair[1]) else { continue };
				if result.elements().last().and_then(|element| element.end_point()) != Some(start) {
					result.move_to(start);
				}
				result.line_to(end);
			}
		}
	}

	Cow::Owned(result)
}

/// The point where the line from `inside` to `outside` crosses one of the bounding half-planes, landing exactly on it.
fn bound_crossing(inside: Point, outside: Point, (axis, sign): (usize, f64)) -> Point {
	let coordinate = |point: Point| sign * if axis == 0 { point.x } else { point.y };
	let t = (MAX_PDF_COORDINATE - coordinate(inside)) / (coordinate(outside) - coordinate(inside));

	// Interpolated from the inside point, which stays precise when the outside one is unboundedly far
	let mut crossing = inside + (outside - inside) * t;
	if axis == 0 {
		crossing.x = sign * MAX_PDF_COORDINATE;
	} else {
		crossing.y = sign * MAX_PDF_COORDINATE;
	}
	crossing
}

/// A polygon clipped to the bounding half-planes by Sutherland-Hodgman clipping, in O(vertices).
fn clipped_polygon(mut vertices: Vec<Point>) -> Vec<Point> {
	for half_plane in BOUNDING_HALF_PLANES {
		let (axis, sign) = half_plane;
		let inside = |point: Point| sign * if axis == 0 { point.x } else { point.y } <= MAX_PDF_COORDINATE;

		let mut clipped = Vec::with_capacity(vertices.len() + 4);
		for (index, &current) in vertices.iter().enumerate() {
			let previous = vertices[(index + vertices.len() - 1) % vertices.len()];
			match (inside(previous), inside(current)) {
				(true, true) => clipped.push(current),
				(true, false) => clipped.push(bound_crossing(previous, current, half_plane)),
				(false, true) => {
					clipped.push(bound_crossing(current, previous, half_plane));
					clipped.push(current);
				}
				(false, false) => {}
			}
		}
		vertices = clipped;
	}
	vertices
}

/// A line segment clipped to the bounding half-planes, or `None` if none of it lies within them.
fn clipped_segment(mut start: Point, mut end: Point) -> Option<(Point, Point)> {
	for half_plane in BOUNDING_HALF_PLANES {
		let (axis, sign) = half_plane;
		let inside = |point: Point| sign * if axis == 0 { point.x } else { point.y } <= MAX_PDF_COORDINATE;

		match (inside(start), inside(end)) {
			(true, true) => {}
			(true, false) => end = bound_crossing(start, end, half_plane),
			(false, true) => start = bound_crossing(end, start, half_plane),
			(false, false) => return None,
		}
	}
	Some((start, end))
}

fn pdf_fill_rule(fill_rule: Fill) -> FillRule {
	match fill_rule {
		Fill::NonZero => FillRule::NonZero,
		Fill::EvenOdd => FillRule::EvenOdd,
	}
}

/// Every one of the separable and non-separable blend modes Vello draws with has its counterpart in a PDF.
fn pdf_blend_mode(mix: Mix) -> PdfBlendMode {
	match mix {
		Mix::Normal => PdfBlendMode::Normal,
		Mix::Multiply => PdfBlendMode::Multiply,
		Mix::Screen => PdfBlendMode::Screen,
		Mix::Overlay => PdfBlendMode::Overlay,
		Mix::Darken => PdfBlendMode::Darken,
		Mix::Lighten => PdfBlendMode::Lighten,
		Mix::ColorDodge => PdfBlendMode::ColorDodge,
		Mix::ColorBurn => PdfBlendMode::ColorBurn,
		Mix::HardLight => PdfBlendMode::HardLight,
		Mix::SoftLight => PdfBlendMode::SoftLight,
		Mix::Difference => PdfBlendMode::Difference,
		Mix::Exclusion => PdfBlendMode::Exclusion,
		Mix::Hue => PdfBlendMode::Hue,
		Mix::Saturation => PdfBlendMode::Saturation,
		Mix::Color => PdfBlendMode::Color,
		Mix::Luminosity => PdfBlendMode::Luminosity,
	}
}

fn pdf_stroke(stroke: &Stroke, paint: Paint, opacity: NormalizedF32) -> PdfStroke {
	// A PDF has one cap style for both ends, which the renderer always gives a stroke anyway
	let line_cap = match stroke.start_cap {
		Cap::Butt => LineCap::Butt,
		Cap::Round => LineCap::Round,
		Cap::Square => LineCap::Square,
	};
	let line_join = match stroke.join {
		Join::Bevel => LineJoin::Bevel,
		Join::Miter => LineJoin::Miter,
		Join::Round => LineJoin::Round,
	};

	// An odd-length pattern is doubled, as SVG and kurbo do, so dashes and gaps alternate the same in every viewer
	let repetitions = if stroke.dash_pattern.len() % 2 == 1 { 2 } else { 1 };
	let dash_length = stroke.dash_pattern.iter().sum::<f64>() * repetitions as f64;
	let dash = (dash_length > 0. && (dash_length as f32).is_finite() && stroke.dash_pattern.iter().all(|length| *length >= 0.)).then(|| StrokeDash {
		array: stroke.dash_pattern.iter().cycle().take(stroke.dash_pattern.len() * repetitions).map(|length| *length as f32).collect(),
		// Brought within one pattern's length, where every viewer agrees on where the pattern starts
		offset: stroke.dash_offset.rem_euclid(dash_length) as f32,
	});

	PdfStroke {
		paint,
		width: stroke.width as f32,
		// A PDF can't hold a miter limit below one, which miters the same way as one anyway
		miter_limit: stroke.miter_limit.clamp(1., f32::MAX as f64) as f32,
		line_cap,
		line_join,
		opacity,
		dash,
	}
}

fn pdf_color_components(color: AlphaColor<Srgb>) -> (rgb::Color, NormalizedF32) {
	let [red, green, blue, alpha] = color.components;
	let channel = |value: f32| (value.clamp(0., 1.) * 255.).round() as u8;
	(rgb::Color::new(channel(red), channel(green), channel(blue)), normalized(alpha))
}

fn pdf_stops(stops: &ColorStops) -> Vec<Stop> {
	let mut previous_offset = 0.;
	stops
		.iter()
		.map(|&(offset, color)| {
			// A PDF's stops must not go backward
			previous_offset = offset.max(previous_offset).min(1.);
			let (color, opacity) = pdf_color_components(color);
			Stop {
				offset: normalized(previous_offset as f32),
				color: color.into(),
				opacity,
			}
		})
		.collect()
}

fn pdf_linear_gradient(start: Point, end: Point, transform: PdfTransform, stops: &ColorStops) -> Paint {
	LinearGradient {
		x1: start.x as f32,
		y1: start.y as f32,
		x2: end.x as f32,
		y2: end.y as f32,
		transform,
		spread_method: SpreadMethod::Pad,
		stops: pdf_stops(stops),
		anti_alias: false,
	}
	.into()
}

fn pdf_radial_gradient(position: peniko::RadialGradientPosition, transform: PdfTransform, stops: &ColorStops) -> Paint {
	RadialGradient {
		fx: position.start_center.x as f32,
		fy: position.start_center.y as f32,
		fr: position.start_radius,
		cx: position.end_center.x as f32,
		cy: position.end_center.y as f32,
		cr: position.end_radius,
		transform,
		spread_method: SpreadMethod::Pad,
		stops: pdf_stops(stops),
		anti_alias: false,
	}
	.into()
}

/// Whether Vello would paint a gradient as its first color for having geometry that's too degenerate to spread colors across.
fn gradient_is_degenerate(gradient: &peniko::Gradient) -> bool {
	let near = |a: Point, b: Point| (a.x - b.x).abs() <= DEGENERATE_GRADIENT_THRESHOLD && (a.y - b.y).abs() <= DEGENERATE_GRADIENT_THRESHOLD;
	match gradient.kind {
		GradientKind::Linear(position) => near(position.start, position.end),
		GradientKind::Radial(position) => {
			position.start_radius < 0.
				|| position.end_radius < 0.
				|| (near(position.start_center, position.end_center) && ((position.end_radius - position.start_radius).abs() as f64) <= DEGENERATE_GRADIENT_THRESHOLD)
		}
		GradientKind::Sweep(position) => position.end_angle <= position.start_angle + DEGENERATE_GRADIENT_THRESHOLD as f32,
	}
}

/// The gradient's stops as straight-alpha sRGB colors, which is how a PDF interpolates them, resampled finely enough to follow any other interpolation.
fn straight_srgb_stops(gradient: &peniko::Gradient) -> ColorStops {
	let stops = &gradient.stops.0;
	let srgb = |stop: &peniko::ColorStop| (stop.offset as f64, stop.color.to_alpha_color::<Srgb>());

	let interpolates_like_pdf = gradient.interpolation_cs == ColorSpaceTag::Srgb;
	let premultiplied = gradient.interpolation_alpha_space == InterpolationAlphaSpace::Premultiplied;

	let mut result = Vec::with_capacity(stops.len());
	for (index, stop) in stops.iter().enumerate() {
		result.push(srgb(stop));

		let Some(next) = stops.get(index + 1) else { break };
		// Premultiplying alpha only changes a span with ends of differing alpha
		let alphas_differ = stop.color.components[3] != next.color.components[3];
		if interpolates_like_pdf && !(premultiplied && alphas_differ) {
			continue;
		}

		for step in 1..RESAMPLED_STOPS_PER_SPAN {
			let t = step as f32 / RESAMPLED_STOPS_PER_SPAN as f32;
			let color = if premultiplied {
				stop.color.interpolate(next.color, gradient.interpolation_cs, gradient.hue_direction).eval(t)
			} else {
				stop.color.interpolate_unpremultiplied(next.color, gradient.interpolation_cs, gradient.hue_direction).eval(t)
			};
			let offset = stop.offset + (next.offset - stop.offset) * t;
			result.push((offset as f64, color.to_alpha_color::<Srgb>()));
		}
	}
	result
}

/// The least and greatest of some values.
fn range_of(values: impl Iterator<Item = f64>) -> (f64, f64) {
	values.fold((f64::INFINITY, f64::NEG_INFINITY), |(least, greatest), value| (least.min(value), greatest.max(value)))
}

/// Whether the stops begin or end with two differing colors at the same position, a hard edge right at the end of the gradient's range.
fn ends_on_hard_edge(stops: &ColorStops) -> bool {
	let hard_edge = |(position, color): &(f64, AlphaColor<Srgb>), (other_position, other_color): &(f64, AlphaColor<Srgb>)| position == other_position && color != other_color;
	matches!(stops.as_slice(), [first, second, ..] if hard_edge(first, second)) || matches!(stops.as_slice(), [.., second_last, last] if hard_edge(second_last, last))
}

/// The stops of a gradient spread across `range_start..range_end` of its positions as its extend mode spreads them, with offsets within that range.
/// `None` if there are too many repetitions to write out.
fn stops_across_range(stops: &ColorStops, extend: Extend, range_start: f64, range_end: f64) -> Option<ColorStops> {
	let range = range_end - range_start;
	if !range.is_finite() || range <= 0. {
		return None;
	}

	let positioned = if extend == Extend::Pad {
		stops.clone()
	} else {
		let repetitions = range_end.ceil() - range_start.floor();
		if repetitions * (stops.len() + 2) as f64 > MAX_REPEATED_GRADIENT_STOPS as f64 {
			return None;
		}
		let (first_repetition, last_repetition) = (range_start.floor() as i64, range_end.ceil() as i64);
		let repetitions = repetitions as usize;

		// Each repetition spans its whole length, from the color at its start to the one at its end
		let (first_color, last_color) = (stops.first()?.1, stops.last()?.1);
		let span: ColorStops = std::iter::once((0., first_color)).chain(stops.iter().copied()).chain(std::iter::once((1., last_color))).collect();

		let mut positioned = Vec::with_capacity(repetitions * span.len());
		for repetition in first_repetition..last_repetition {
			let reflected = extend == Extend::Reflect && repetition.rem_euclid(2) == 1;
			let base = repetition as f64;
			if reflected {
				positioned.extend(span.iter().rev().map(|&(offset, color)| (base + 1. - offset, color)));
			} else {
				positioned.extend(span.iter().map(|&(offset, color)| (base + offset, color)));
			}
		}
		positioned.dedup();
		positioned
	};

	// Clip to the range, which needn't begin or end on a whole repetition, taking each end's color from inside the range
	let mut clipped = vec![(0., sample_stops(&positioned, range_start, false))];
	clipped.extend(
		positioned
			.iter()
			.filter(|(position, _)| *position > range_start && *position < range_end)
			.map(|&(position, color)| ((position - range_start) / range, color)),
	);
	clipped.push((1., sample_stops(&positioned, range_end, true)));
	Some(clipped)
}

/// The color at `position` along stops at ascending positions, interpolated in straight-alpha sRGB as a PDF does, extending their end colors beyond them.
/// At a hard edge, it's the color on the side `position` is approached from.
fn sample_stops(stops: &ColorStops, position: f64, approached_from_below: bool) -> AlphaColor<Srgb> {
	let after = stops.partition_point(|(stop_position, _)| if approached_from_below { *stop_position < position } else { *stop_position <= position });
	match (after.checked_sub(1).and_then(|before| stops.get(before)), stops.get(after)) {
		(Some(&(start, start_color)), Some(&(end, end_color))) if end > start => {
			let t = ((position - start) / (end - start)) as f32;
			AlphaColor::new(std::array::from_fn(|channel| {
				start_color.components[channel] + (end_color.components[channel] - start_color.components[channel]) * t
			}))
		}
		(Some(&(_, color)), _) | (None, Some(&(_, color))) => color,
		(None, None) => AlphaColor::TRANSPARENT,
	}
}

/// Where a point in a gradient's own space falls along it, or `None` where the gradient paints nothing.
fn gradient_position(kind: &GradientKind, point: Point) -> Option<f64> {
	match *kind {
		GradientKind::Linear(position) => {
			let direction = position.end - position.start;
			Some((point - position.start).dot(direction) / direction.hypot2())
		}
		GradientKind::Radial(position) => {
			// The largest position with a circle passing through the point, among circles of nonnegative radius interpolated between the two
			let center_offset = position.end_center - position.start_center;
			let point_offset = point - position.start_center;
			let (start_radius, radius_growth) = (position.start_radius as f64, (position.end_radius - position.start_radius) as f64);

			let a = center_offset.hypot2() - radius_growth * radius_growth;
			let b = point_offset.dot(center_offset) + start_radius * radius_growth;
			let c = point_offset.hypot2() - start_radius * start_radius;
			let valid = |t: f64| start_radius + radius_growth * t >= 0.;

			if a.abs() <= f64::EPSILON {
				let t = c / (2. * b);
				return (t.is_finite() && valid(t)).then_some(t);
			}

			let discriminant = b * b - a * c;
			if discriminant < 0. {
				return None;
			}
			let root = discriminant.sqrt();
			let (larger, smaller) = (((b + root) / a).max((b - root) / a), ((b + root) / a).min((b - root) / a));
			[larger, smaller].into_iter().find(|t| valid(*t))
		}
		GradientKind::Sweep(position) => {
			let offset = point - position.center;
			let angle = offset.y.atan2(offset.x).rem_euclid(std::f64::consts::TAU);
			Some((angle - position.start_angle as f64) / (position.end_angle - position.start_angle) as f64)
		}
	}
}

/// An image's RGBA8 pixels with straight alpha, as a PDF holds them.
fn straight_rgba8(data: &peniko::ImageData) -> Option<(Vec<u8>, u32, u32)> {
	let length = data.width as usize * data.height as usize * 4;
	let mut rgba = data.data.data().get(..length)?.to_vec();

	for pixel in rgba.as_chunks_mut::<4>().0 {
		if data.format == ImageFormat::Bgra8 {
			pixel.swap(0, 2);
		}

		let alpha = pixel[3] as u32;
		if data.alpha_type == ImageAlphaType::AlphaPremultiplied && alpha > 0 && alpha < 255 {
			for channel in &mut pixel[..3] {
				*channel = ((*channel as u32 * 255 + alpha / 2) / alpha).min(255) as u8;
			}
		}
	}

	Some((rgba, data.width, data.height))
}

/// Straight-alpha RGBA8 pixels as an image to embed, leaving out the alpha channel when every pixel is opaque.
fn pdf_embedded_image(rgba: Vec<u8>, width: u32, height: u32, interpolate: bool) -> Option<PdfEmbeddedImage> {
	let pixel_count = width as usize * height as usize;
	if pixel_count == 0 || rgba.len() != pixel_count * 4 {
		return None;
	}

	let (pixels, _) = rgba.as_chunks::<4>();
	let color = pixels.iter().flat_map(|pixel| [pixel[0], pixel[1], pixel[2]]).collect();
	let alpha = pixels.iter().any(|pixel| pixel[3] < 255).then(|| pixels.iter().map(|pixel| pixel[3]).collect());

	let image = PdfImagePixels {
		color: Arc::new(color),
		alpha: alpha.map(Arc::new),
		width,
		height,
	};
	PdfEmbeddedImage::from_custom(image, interpolate).ok()
}

/// Pixels in the separate color and alpha channels a PDF image holds, hashed by their contents so krilla embeds identical ones once.
#[derive(Clone, Hash)]
struct PdfImagePixels {
	color: Arc<Vec<u8>>,
	alpha: Option<Arc<Vec<u8>>>,
	width: u32,
	height: u32,
}

impl CustomImage for PdfImagePixels {
	fn color_channel(&self) -> &[u8] {
		&self.color
	}

	fn alpha_channel(&self) -> Option<&[u8]> {
		self.alpha.as_deref().map(Vec::as_slice)
	}

	fn bits_per_component(&self) -> BitsPerComponent {
		BitsPerComponent::Eight
	}

	fn size(&self) -> (u32, u32) {
		(self.width, self.height)
	}

	fn icc_profile(&self) -> Option<&[u8]> {
		None
	}

	fn color_space(&self) -> ImageColorspace {
		ImageColorspace::Rgb
	}
}

#[cfg(test)]
mod tests;
