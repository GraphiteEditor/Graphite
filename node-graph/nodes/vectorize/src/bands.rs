//! A path gradient drawn with Graphite's paints: the color at signed distance `d` from a curve becomes strokes along the
//! curve, a stroke of width `2r` covering exactly `|d| < r`, stacked widest first on each side of the curve and masked to
//! that side and to the object. Ported from vtrace's `emit` crate at its finest level, which needs no blur.

use crate::convert::{
	ATTR_VTRACE_BLUR, ATTR_VTRACE_PATH_GRADIENT, PathGradient, VectorItemExt, color, cubics_to_bezpath, gradient_stops, group, loops_to_bezpath, mask_base, vector_item, with_clipping_mask,
};
use core_types::Color;
use core_types::list::{ATTR_OPACITY, Item};
use diffrast::Gradient;
use glam::{DVec2, DVec3, Vec3};
use graphic_types::{Graphic, Vector};
use kurbo::BezPath;
use vector_types::vector::style::{Stroke, StrokeJoin};
use vtrace::model::{Geometry, Object};

/// Bands per side of the curve, enough that their steps are below what the eye picks out.
const BANDS: usize = 32;
/// Margin in pixels around the object that the background rectangles extend over, so their edges never show.
const PADDING: f64 = 2.;

/// The profile: the color at signed distance `d` from the curve.
struct Profile<'a> {
	gradient: &'a Gradient,
	from: Vec3,
	near: f32,
	far: f32,
}

impl Profile<'_> {
	fn t(&self, d: f32) -> f32 {
		((d - self.near) / (self.far - self.near).max(1e-6)).clamp(0., 1.)
	}

	fn at(&self, d: f32) -> Color {
		let (stop_color, alpha) = self.gradient.rgba(self.from, self.t(d));
		color(stop_color.as_dvec3(), alpha as f64)
	}

	/// The `bands + 1` breakpoints from `near` to `far` that cut the profile into bands of equal color change, so the steep stretches get the narrow bands.
	fn breaks(&self, bands: usize) -> Vec<f32> {
		let knots = self.knots();
		let mut steps: Vec<f32> = knots.windows(2).map(|pair| pair[0].1.distance(pair[1].1)).collect();
		if !self.gradient.is_opaque() {
			for (step, alphas) in steps.iter_mut().zip(self.alphas().windows(2)) {
				*step += (alphas[1] - alphas[0]).abs();
			}
		}

		let total: f32 = steps.iter().sum();
		if total <= 0. {
			let step = (self.far - self.near) / bands as f32;
			return (0..=bands).map(|i| self.near + step * i as f32).collect();
		}

		(0..=bands)
			.map(|i| {
				let mut wanted = total * i as f32 / bands as f32;
				for (k, &step) in steps.iter().enumerate() {
					if wanted <= step || k + 1 == steps.len() {
						let u = (wanted / step.max(1e-9)).clamp(0., 1.);
						return knots[k].0 + u * (knots[k + 1].0 - knots[k].0);
					}
					wanted -= step;
				}
				self.far
			})
			.collect()
	}

	/// The ends and the stops as distances and colors, premultiplied where the ramp has alpha.
	fn knots(&self) -> Vec<(f32, Vec3)> {
		let width = self.far - self.near;
		let opaque = self.gradient.is_opaque();
		let knot_color = |c: [f32; 4]| match opaque {
			true => glam::Vec4::from(c).truncate(),
			false => glam::Vec4::from(c).truncate() * c[3],
		};

		std::iter::once((self.near, self.from))
			.chain(self.gradient.stops().iter().map(|(t, c)| (self.near + t * width, knot_color(c))))
			.chain(std::iter::once((self.far, knot_color(self.gradient.to()))))
			.collect()
	}

	/// The knots' alphas, in the order of [`Profile::knots`].
	fn alphas(&self) -> Vec<f32> {
		std::iter::once(1.)
			.chain(self.gradient.stops().iter().map(|(_, c)| c[3]))
			.chain(std::iter::once(self.gradient.to()[3]))
			.collect()
	}
}

/// Draws `object`, painted with the path gradient `gradient` starting from `from`, as a group masked to the object's shape.
pub fn path_gradient(object: &Object, from: DVec3, gradient: &Gradient) -> Item<Graphic> {
	let Gradient::Path { contour, near, far, .. } = gradient else {
		return group(Vec::new());
	};
	let Geometry::Fill { loops, rule } = &object.geometry else {
		log::error!("Vectorize only draws path gradients on filled shapes");
		return group(Vec::new());
	};
	if !gradient.is_opaque() {
		log::warn!("Vectorize approximates a path gradient with alpha by translucent bands, which overlap where they stack");
	}

	let profile = Profile {
		gradient,
		from: from.as_vec3(),
		near: *near,
		far: *far,
	};
	let breaks = profile.breaks(BANDS);

	// What the strokes run along (an open curve extended by its rays to the box), and the region of negative distance (that curve closed by the box)
	let cubics: Vec<[DVec2; 4]> = contour.cubics().map(|cubic| cubic.map(|p| p.as_dvec2())).collect();
	let (curve, region) = if contour.is_closed() {
		let closed = cubics_to_bezpath(&cubics, true);
		(closed.clone(), closed)
	} else {
		let closure: Vec<kurbo::Point> = contour.closure().iter().map(|p| kurbo::Point::new(p.x as f64, p.y as f64)).collect();
		let (Some(&ray_end), Some(&ray_start)) = (closure.first(), closure.last()) else {
			return group(Vec::new());
		};
		let open = cubics_to_bezpath(&cubics, false);

		let mut curve = BezPath::new();
		curve.move_to(ray_start);
		open.elements()
			.iter()
			.map(|&element| if let kurbo::PathEl::MoveTo(point) = element { kurbo::PathEl::LineTo(point) } else { element })
			.for_each(|element| curve.push(element));
		curve.line_to(ray_end);

		let mut region = open;
		closure.iter().for_each(|&point| region.line_to(point));
		region.close_path();

		(curve, region)
	};

	let (min, max) = loops.iter().flatten().flatten().fold((DVec2::INFINITY, DVec2::NEG_INFINITY), |(min, max), &p| (min.min(p), max.max(p)));
	let rectangle = kurbo::Rect::new(min.x - PADDING, min.y - PADDING, max.x + PADDING, max.y + PADDING);

	// The outside is drawn whole and the inside over it masked to the region, so the curve's anti-aliasing lets nothing through
	let outside = side(&profile, &breaks, 1., &curve, rectangle);
	let inside_content = side(&profile, &breaks, -1., &curve, rectangle);
	let inside = group(vec![mask_base(region), with_clipping_mask(inside_content)]);
	let bands = group(vec![outside, inside]);

	let mut drawn = group(vec![mask_base(loops_to_bezpath(loops, *rule)), with_clipping_mask(bands)]);
	if object.opacity < 1. {
		drawn.set_attribute(ATTR_OPACITY, object.opacity);
	}
	if object.blur > 0. {
		drawn.set_attribute(ATTR_VTRACE_BLUR, object.blur);
	}

	let path_gradient = PathGradient {
		contour: Vector::from_bezpath(curve),
		closed: contour.is_closed(),
		near: *near as f64,
		far: *far as f64,
		stops: gradient_stops(from, gradient),
	};
	drawn.with_attribute(ATTR_VTRACE_PATH_GRADIENT, path_gradient)
}

/// One side of the curve (`sign` 1 outside, -1 inside): its far value over the box, then a stroke per breakpoint on it, widest first, each in the value of the band inside it.
fn side(profile: &Profile, breaks: &[f32], sign: f32, curve: &BezPath, rectangle: kurbo::Rect) -> Item<Graphic> {
	let mut radii: Vec<f32> = breaks.iter().map(|d| sign * d).filter(|&radius| radius > 0.).collect();
	radii.sort_by(|a, b| b.total_cmp(a));

	let background = vector_item(kurbo::Shape::to_path(&rectangle, 0.1)).into_fill(Graphic::from(profile.at(sign * f32::INFINITY)));
	let mut items = vec![Item::new_from_element(Graphic::from(background))];

	for (k, &radius) in radii.iter().enumerate() {
		let inner = radii.get(k + 1).copied().unwrap_or(0.);
		let band_color = profile.at(sign * 0.5 * (radius + inner));
		let stroke = Stroke {
			join: StrokeJoin::Round,
			..Stroke::new(2. * radius as f64)
		};
		let band = vector_item(curve.clone()).into_stroke(&stroke, Graphic::from(band_color));
		items.push(Item::new_from_element(Graphic::from(band)));
	}

	group(items)
}
