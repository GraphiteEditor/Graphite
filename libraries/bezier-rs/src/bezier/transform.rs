use super::*;

use crate::compare::compare_points;
use crate::utils::{f64_compare, Cap, TValue};
use crate::{AppendType, ManipulatorGroup, Subpath};

use glam::DMat2;
use std::f64::consts::PI;

/// Functionality that transform Beziers, such as split, reduce, offset, etc.
impl Bezier {
	/// Returns a linear approximation of the given [Bezier]. For higher order [Bezier], this means simply dropping the handles.
	pub fn to_linear(&self) -> Bezier {
		Bezier::from_linear_dvec2(self.start(), self.end())
	}

	/// Returns a quadratic approximation of the given [Bezier]. For cubic Bezier, which typically cannot be represented by a single
	/// quadratic segment, this function simply takes the average of the cubic handles to be the new quadratic handle.
	pub fn to_quadratic(&self) -> Bezier {
		let handle = match self.handles {
			BezierHandles::Linear => self.start,
			BezierHandles::Quadratic { handle } => handle,
			BezierHandles::Cubic { handle_start, handle_end } => (handle_start + handle_end) / 2.,
		};
		Bezier::from_quadratic_dvec2(self.start, handle, self.end)
	}

	/// Returns a cubic approximation of the given [Bezier].
	pub fn to_cubic(&self) -> Bezier {
		let (handle_start, handle_end) = match self.handles {
			BezierHandles::Linear => (self.start, self.end),
			// Conversion reference source: https://stackoverflow.com/a/63059651/775283
			BezierHandles::Quadratic { handle } => (self.start + (2. / 3.) * (handle - self.start), self.end + (2. / 3.) * (handle - self.end)),
			BezierHandles::Cubic { handle_start: _, handle_end: _ } => return *self,
		};
		Bezier::from_cubic_dvec2(self.start, handle_start, handle_end, self.end)
	}

	/// Returns the pair of Bezier curves that result from splitting the original curve at the point `t` along the curve.
	/// <iframe frameBorder="0" width="100%" height="350px" src="https://graphite.rs/libraries/bezier-rs#bezier/split/solo" title="Split Demo"></iframe>
	pub fn split(&self, t: TValue) -> [Bezier; 2] {
		let t = self.t_value_to_parametric(t);
		let split_point = self.evaluate(TValue::Parametric(t));

		match self.handles {
			BezierHandles::Linear => [Bezier::from_linear_dvec2(self.start, split_point), Bezier::from_linear_dvec2(split_point, self.end)],
			// TODO: Actually calculate the correct handle locations
			BezierHandles::Quadratic { handle } => {
				let t_minus_one = t - 1.;
				[
					Bezier::from_quadratic_dvec2(self.start, t * handle - t_minus_one * self.start, split_point),
					Bezier::from_quadratic_dvec2(split_point, t * self.end - t_minus_one * handle, self.end),
				]
			}
			BezierHandles::Cubic { handle_start, handle_end } => {
				let t_minus_one = t - 1.;
				[
					Bezier::from_cubic_dvec2(
						self.start,
						t * handle_start - t_minus_one * self.start,
						(t * t) * handle_end - 2. * t * t_minus_one * handle_start + (t_minus_one * t_minus_one) * self.start,
						split_point,
					),
					Bezier::from_cubic_dvec2(
						split_point,
						(t * t) * self.end - 2. * t * t_minus_one * handle_end + (t_minus_one * t_minus_one) * handle_start,
						t * self.end - t_minus_one * handle_end,
						self.end,
					),
				]
			}
		}
	}

	/// Returns a reversed version of the Bezier curve.
	pub fn reverse(&self) -> Bezier {
		match self.handles {
			BezierHandles::Linear => Bezier::from_linear_dvec2(self.end, self.start),
			BezierHandles::Quadratic { handle } => Bezier::from_quadratic_dvec2(self.end, handle, self.start),
			BezierHandles::Cubic { handle_start, handle_end } => Bezier::from_cubic_dvec2(self.end, handle_end, handle_start, self.start),
		}
	}

	/// Returns the Bezier curve representing the sub-curve between the two provided points.
	/// It will start at the point corresponding to the smaller of `t1` and `t2`, and end at the point corresponding to the larger of `t1` and `t2`.
	/// <iframe frameBorder="0" width="100%" height="400px" src="https://graphite.rs/libraries/bezier-rs#bezier/trim/solo" title="Trim Demo"></iframe>
	pub fn trim(&self, t1: TValue, t2: TValue) -> Bezier {
		let (mut t1, mut t2) = (self.t_value_to_parametric(t1), self.t_value_to_parametric(t2));
		// If t1 is equal to t2, return a bezier comprised entirely of the same point
		if f64_compare(t1, t2, MAX_ABSOLUTE_DIFFERENCE) {
			let point = self.evaluate(TValue::Parametric(t1));
			return match self.handles {
				BezierHandles::Linear => Bezier::from_linear_dvec2(point, point),
				BezierHandles::Quadratic { handle: _ } => Bezier::from_quadratic_dvec2(point, point, point),
				BezierHandles::Cubic { handle_start: _, handle_end: _ } => Bezier::from_cubic_dvec2(point, point, point, point),
			};
		} else if t1 > t2 {
			(t1, t2) = (t2, t1)
		}
		let bezier_ending_at_t2 = self.split(TValue::Parametric(t2))[0];
		// Adjust the ratio `t1` to its corresponding value on the new curve that was split on `t2`
		let adjusted_t1 = t1 / t2;
		bezier_ending_at_t2.split(TValue::Parametric(adjusted_t1))[1]
	}

	/// Returns a Bezier curve that results from applying the transformation function to each point in the Bezier.
	pub fn apply_transformation(&self, transformation_function: impl Fn(DVec2) -> DVec2) -> Bezier {
		let transformed_start = transformation_function(self.start);
		let transformed_end = transformation_function(self.end);
		match self.handles {
			BezierHandles::Linear => Bezier::from_linear_dvec2(transformed_start, transformed_end),
			BezierHandles::Quadratic { handle } => {
				let transformed_handle = transformation_function(handle);
				Bezier::from_quadratic_dvec2(transformed_start, transformed_handle, transformed_end)
			}
			BezierHandles::Cubic { handle_start, handle_end } => {
				let transformed_handle_start = transformation_function(handle_start);
				let transformed_handle_end = transformation_function(handle_end);
				Bezier::from_cubic_dvec2(transformed_start, transformed_handle_start, transformed_handle_end, transformed_end)
			}
		}
	}

	/// Returns a Bezier curve that results from rotating the curve around the origin by the given angle (in radians).
	/// <iframe frameBorder="0" width="100%" height="325px" src="https://graphite.rs/libraries/bezier-rs#bezier/rotate/solo" title="Rotate Demo"></iframe>
	pub fn rotate(&self, angle: f64) -> Bezier {
		let rotation_matrix = DMat2::from_angle(angle);
		self.apply_transformation(|point| rotation_matrix.mul_vec2(point))
	}

	/// Returns a Bezier curve that results from rotating the curve around the provided point by the given angle (in radians).
	pub fn rotate_about_point(&self, angle: f64, pivot: DVec2) -> Bezier {
		let rotation_matrix = DMat2::from_angle(angle);
		self.apply_transformation(|point| rotation_matrix.mul_vec2(point - pivot) + pivot)
	}

	/// Returns a Bezier curve that results from translating the curve by the given `DVec2`.
	pub fn translate(&self, translation: DVec2) -> Bezier {
		self.apply_transformation(|point| point + translation)
	}

	/// Determine if it is possible to scale the given curve, using the following conditions:
	/// 1. All the handles are located on a single side of the curve.
	/// 2. The on-curve point for `t = 0.5` must occur roughly in the center of the polygon defined by the curve's endpoint normals.
	/// See [the offset section](https://pomax.github.io/bezierinfo/#offsetting) of Pomax's bezier curve primer for more details.
	fn is_scalable(&self) -> bool {
		if self.handles == BezierHandles::Linear {
			return true;
		}
		// Verify all the handles are located on a single side of the curve.
		if let BezierHandles::Cubic { handle_start, handle_end } = self.handles {
			let angle_1 = (self.end - self.start).angle_between(handle_start - self.start);
			let angle_2 = (self.end - self.start).angle_between(handle_end - self.start);
			if (angle_1 > 0. && angle_2 < 0.) || (angle_1 < 0. && angle_2 > 0.) {
				return false;
			}
		}
		// Verify the angle formed by the endpoint normals is sufficiently small, ensuring the on-curve point for `t = 0.5` occurs roughly in the center of the polygon.
		let normal_0 = self.normal(TValue::Parametric(0.));
		let normal_1 = self.normal(TValue::Parametric(1.));
		let endpoint_normal_angle = (normal_0.x * normal_1.x + normal_0.y * normal_1.y).min(1.).acos();
		endpoint_normal_angle < SCALABLE_CURVE_MAX_ENDPOINT_NORMAL_ANGLE
	}

	/// Add the bezier endpoints if not already present, and combine and sort the dimensional extrema.
	pub(crate) fn get_extrema_t_list(&self) -> Vec<f64> {
		let mut extrema = self.local_extrema().into_iter().flatten().collect::<Vec<f64>>();
		extrema.append(&mut vec![0., 1.]);
		extrema.sort_by(|ex1, ex2| ex1.partial_cmp(ex2).unwrap());
		extrema.dedup();
		extrema
	}

	/// Returns a tuple of the scalable subcurves and the corresponding `t` values that were used to split the curve.
	/// This function may introduce gaps if subsections of the curve are not reducible.
	/// The function takes the following parameter:
	/// - `step_size` - Dictates the granularity at which the function searches for reducible subcurves. The default value is `0.01`.
	///   A small granularity may increase the chance the function does not introduce gaps, but will increase computation time.
	pub(crate) fn reduced_curves_and_t_values(&self, step_size: Option<f64>) -> (Vec<Bezier>, Vec<[f64; 2]>) {
		// A linear segment is scalable, so return itself
		if let BezierHandles::Linear = self.handles {
			return (vec![*self], vec![[0., 1.]]);
		}

		let step_size = step_size.unwrap_or(DEFAULT_REDUCE_STEP_SIZE);

		let mut extrema = self.get_extrema_t_list();
		if let BezierHandles::Cubic { handle_start: _, handle_end: _ } = self.handles {
			extrema.append(&mut self.inflections());
			extrema.sort_by(|ex1, ex2| ex1.partial_cmp(ex2).unwrap());
		}

		// Split each subcurve such that each resulting segment is scalable.
		let mut result_beziers: Vec<Bezier> = Vec::new();
		let mut result_t_values: Vec<[f64; 2]> = vec![];

		extrema.windows(2).for_each(|t_pair| {
			let t_subcurve_start = t_pair[0];
			let t_subcurve_end = t_pair[1];
			let subcurve = self.trim(TValue::Parametric(t_subcurve_start), TValue::Parametric(t_subcurve_end));
			// Perform no processing on the subcurve if it's already scalable.
			if subcurve.is_scalable() {
				result_beziers.push(subcurve);
				result_t_values.push([t_subcurve_start, t_subcurve_end]);
				return;
			}

			// Greedily iterate across the subcurve at intervals of size `step_size` to break up the curve into maximally large segments
			let mut segment: Bezier;
			let mut t1 = 0.;
			let mut t2 = step_size;
			let mut is_prev_valid = false;
			while t2 <= 1. + step_size {
				segment = subcurve.trim(TValue::Parametric(t1), TValue::Parametric(f64::min(t2, 1.)));
				if !segment.is_scalable() {
					t2 -= step_size;

					// If the previous step does not exist, the start of the subcurve is irreducible.
					// Otherwise, add the valid segment from the previous step to the result.
					if is_prev_valid {
						segment = subcurve.trim(TValue::Parametric(t1), TValue::Parametric(t2));
						if segment.is_scalable() {
							result_beziers.push(segment);
							result_t_values.push([t_subcurve_start + t1 * (t_subcurve_end - t_subcurve_start), t_subcurve_start + t2 * (t_subcurve_end - t_subcurve_start)]);
						} else {
							t2 = t1 + step_size;
						}
					} else {
						t2 = t1 + step_size;
					}
					t1 = t2;
					is_prev_valid = false;
				} else {
					is_prev_valid = true;
				}
				t2 += step_size;
			}
			// Collect final remainder of the curve.
			if t1 < 1. {
				segment = subcurve.trim(TValue::Parametric(t1), TValue::Parametric(1.));
				if segment.is_scalable() {
					result_beziers.push(segment);
					result_t_values.push([t_subcurve_start + t1 * (t_subcurve_end - t_subcurve_start), t_subcurve_end]);
				}
			}
		});
		(result_beziers, result_t_values)
	}

	/// Split the curve into a number of scalable subcurves. This function may introduce gaps if subsections of the curve are not reducible.
	/// The function takes the following parameter:
	/// - `step_size` - Dictates the granularity at which the function searches for reducible subcurves. The default value is `0.01`.
	///   A small granularity may increase the chance the function does not introduce gaps, but will increase computation time.
	/// <iframe frameBorder="0" width="100%" height="300px" src="https://graphite.rs/libraries/bezier-rs#bezier/reduce/solo" title="Reduce Demo"></iframe>
	pub fn reduce(&self, step_size: Option<f64>) -> Vec<Bezier> {
		self.reduced_curves_and_t_values(step_size).0
	}

	/// Scale will translate a bezier curve a fixed distance away from its original position, and stretch/compress the transformed curve to match the translation ratio.
	/// Note that not all bezier curves are possible to scale, so this function asserts that the provided curve is scalable.
	/// A proof for why this is true can be found in the [Curve offsetting section](https://pomax.github.io/bezierinfo/#offsetting) of Pomax's bezier curve primer.
	/// `scale` takes the parameter `distance`, which is the distance away from the curve that the new one will be scaled to. Positive values will scale the curve in the
	/// same direction as the endpoint normals, while negative values will scale in the opposite direction.
	fn scale(&self, distance: f64) -> Bezier {
		assert!(self.is_scalable(), "The curve provided to scale is not scalable. Reduce the curve first.");

		let normal_start = self.normal(TValue::Parametric(0.));
		let normal_end = self.normal(TValue::Parametric(1.));

		// If normal unit vectors are equal, then the lines are parallel
		if normal_start.abs_diff_eq(normal_end, MAX_ABSOLUTE_DIFFERENCE) {
			return self.translate(distance * normal_start);
		}

		// Find the intersection point of the endpoint normals
		let intersection = utils::line_intersection(self.start, normal_start, self.end, normal_end);

		// If the Bezier is a quadratic, convert it to a cubic to increase expressiveness
		let intermediate = match self.handles {
			BezierHandles::Quadratic { handle: _ } => self.to_cubic(),
			_ => *self,
		};

		let should_flip_direction = (self.start - intersection).normalize().abs_diff_eq(normal_start, MAX_ABSOLUTE_DIFFERENCE);
		intermediate.apply_transformation(|point| {
			let mut direction_unit_vector = (intersection - point).normalize();
			if should_flip_direction {
				direction_unit_vector *= -1.;
			}
			point + distance * direction_unit_vector
		})
	}

	/// Version of the `scale` function which scales the curve such that the start of the scaled curve is `start_distance` from the original curve, while the end of
	/// of the scaled curve is `end_distance` from the original curve. The curve transitions from `start_distance` to `end_distance` gradually, proportional to the
	/// distance along the equation (`t`-value) of the curve.
	pub fn graduated_scale(&self, start_distance: f64, end_distance: f64) -> Bezier {
		assert!(self.is_scalable(), "The curve provided to scale is not scalable. Reduce the curve first.");

		// If the Bezier is a quadratic, convert it to a cubic to increase expressiveness
		let intermediate = match self.handles {
			BezierHandles::Quadratic { handle: _ } => self.to_cubic(),
			_ => *self,
		};

		let normal_start = intermediate.normal(TValue::Parametric(0.));
		let normal_end = intermediate.normal(TValue::Parametric(1.));

		// If normal unit vectors are equal, then the lines are parallel
		if normal_start.abs_diff_eq(normal_end, MAX_ABSOLUTE_DIFFERENCE) {
			let transformed_start = utils::scale_point_from_direction_vector(intermediate.start, intermediate.normal(TValue::Parametric(0.)), false, start_distance);
			let transformed_end = utils::scale_point_from_direction_vector(intermediate.end, intermediate.normal(TValue::Parametric(1.)), false, end_distance);

			return match intermediate.handles {
				BezierHandles::Linear => Bezier::from_linear_dvec2(transformed_start, transformed_end),
				BezierHandles::Quadratic { handle: _ } => unreachable!(),
				BezierHandles::Cubic { handle_start, handle_end } => {
					let handle_start_closest_t = intermediate.project(handle_start, None);
					let handle_start_scale_distance = (1. - handle_start_closest_t) * start_distance + handle_start_closest_t * end_distance;
					let transformed_handle_start =
						utils::scale_point_from_direction_vector(handle_start, intermediate.normal(TValue::Parametric(handle_start_closest_t)), false, handle_start_scale_distance);

					let handle_end_closest_t = intermediate.project(handle_start, None);
					let handle_end_scale_distance = (1. - handle_end_closest_t) * start_distance + handle_end_closest_t * end_distance;
					let transformed_handle_end = utils::scale_point_from_direction_vector(handle_end, intermediate.normal(TValue::Parametric(handle_end_closest_t)), false, handle_end_scale_distance);
					Bezier::from_cubic_dvec2(transformed_start, transformed_handle_start, transformed_handle_end, transformed_end)
				}
			};
		}

		// Find the intersection point of the endpoint normals
		let intersection = utils::line_intersection(intermediate.start, normal_start, intermediate.end, normal_end);
		let should_flip_direction = (intermediate.start - intersection).normalize().abs_diff_eq(normal_start, MAX_ABSOLUTE_DIFFERENCE);

		let transformed_start = utils::scale_point_from_origin(intermediate.start, intersection, should_flip_direction, start_distance);
		let transformed_end = utils::scale_point_from_origin(intermediate.end, intersection, should_flip_direction, end_distance);

		match intermediate.handles {
			BezierHandles::Linear => Bezier::from_linear_dvec2(transformed_start, transformed_end),
			BezierHandles::Quadratic { handle: _ } => unreachable!(),
			BezierHandles::Cubic { handle_start, handle_end } => {
				let handle_start_scale_distance = (start_distance * 2. + end_distance) / 3.;
				let transformed_handle_start = utils::scale_point_from_origin(handle_start, intersection, should_flip_direction, handle_start_scale_distance);

				let handle_end_scale_distance = (start_distance + end_distance * 2.) / 3.;
				let transformed_handle_end = utils::scale_point_from_origin(handle_end, intersection, should_flip_direction, handle_end_scale_distance);
				Bezier::from_cubic_dvec2(transformed_start, transformed_handle_start, transformed_handle_end, transformed_end)
			}
		}
	}

	/// Offset will break down the Bezier into reducible subcurves, and scale each subcurve a set distance from the original curve.
	/// Note that not all bezier curves are possible to offset, so this function first reduces the curve to scalable segments and then offsets those segments.
	/// A proof for why this is true can be found in the [Curve offsetting section](https://pomax.github.io/bezierinfo/#offsetting) of Pomax's bezier curve primer.
	/// Offset takes the following parameter:
	/// - `distance` - The offset's distance from the curve. Positive values will offset the curve in the same direction as the endpoint normals,
	/// while negative values will offset in the opposite direction.
	/// <iframe frameBorder="0" width="100%" height="325px" src="https://graphite.rs/libraries/bezier-rs#bezier/offset/solo" title="Offset Demo"></iframe>
	pub fn offset<ManipulatorGroupId: crate::Identifier>(&self, distance: f64) -> Subpath<ManipulatorGroupId> {
		if self.is_point() {
			return Subpath::from_bezier(self);
		}
		let reduced = self.reduce(None);
		let mut scaled = Subpath::new(vec![], false);
		reduced.iter().enumerate().for_each(|(index, bezier)| {
			let scaled_bezier = bezier.scale(distance);
			if !bezier.is_point() {
				if index > 0 && !compare_points(bezier.start(), reduced[index - 1].end()) {
					scaled.append_bezier(&scaled_bezier, AppendType::SmoothJoin(MAX_ABSOLUTE_DIFFERENCE));
				} else {
					scaled.append_bezier(&scaled_bezier, AppendType::IgnoreStart);
				}
			}
		});

		// If the curve is not linear, smooth the handles. All segments produced by bezier::scale will be cubic.
		if self.handles != BezierHandles::Linear {
			scaled.smooth_open_subpath();
		}

		scaled
	}

	/// Version of the `offset` function which scales the offset such that the start of the offset is `start_distance` from the original curve, while the end of
	/// of the offset is `end_distance` from the original curve. The curve transitions from `start_distance` to `end_distance` gradually, proportional to the
	/// distance along the equation (`t`-value) of the curve. Similarly to the `offset` function, the returned result is an approximation.
	pub fn graduated_offset<ManipulatorGroupId: crate::Identifier>(&self, start_distance: f64, end_distance: f64) -> Subpath<ManipulatorGroupId> {
		let reduced = self.reduce(None);
		let mut next_start_distance = start_distance;
		let distance_difference = end_distance - start_distance;
		let total_length = self.length(None);
		if total_length < MAX_ABSOLUTE_DIFFERENCE {
			return Subpath::new(vec![], false);
		}

		let mut result = Subpath::new(vec![], false);
		reduced.iter().enumerate().for_each(|(index, bezier)| {
			if !bezier.is_point() {
				let current_length = bezier.length(None);
				let next_end_distance = next_start_distance + (current_length / total_length) * distance_difference;
				let scaled_bezier = bezier.graduated_scale(next_start_distance, next_end_distance);

				if index > 0 && !compare_points(bezier.start(), reduced[index - 1].end()) {
					result.append_bezier(&scaled_bezier, AppendType::SmoothJoin(MAX_ABSOLUTE_DIFFERENCE));
				} else {
					result.append_bezier(&scaled_bezier, AppendType::IgnoreStart);
				}
				next_start_distance = next_end_distance;
			}
		});

		// If the curve is not linear, smooth the handles. All segments produced by bezier::scale will be cubic.
		if self.handles != BezierHandles::Linear {
			result.smooth_open_subpath();
		}

		result
	}

	/// Outline will return a vector of Beziers that creates an outline around the curve at the designated distance away from the curve.
	/// It makes use of the `offset` function, thus restrictions applicable to `offset` are relevant to this function as well.
	/// The 'caps', the linear segments at opposite ends of the outline, intersect the original curve at the midpoint of the cap.
	/// Outline takes the following parameter:
	/// - `distance` - The outline's distance from the curve.
	/// <iframe frameBorder="0" width="100%" height="350px" src="https://graphite.rs/libraries/bezier-rs#bezier/outline/solo" title="Outline Demo"></iframe>
	pub fn outline<ManipulatorGroupId: crate::Identifier>(&self, distance: f64, cap: Cap) -> Subpath<ManipulatorGroupId> {
		let (pos_offset, neg_offset) = if self.is_point() {
			(
				Subpath::new(vec![ManipulatorGroup::new_anchor(self.start() + DVec2::NEG_Y * distance)], false),
				Subpath::new(vec![ManipulatorGroup::new_anchor(self.start() + DVec2::Y * distance)], false),
			)
		} else {
			(self.offset(distance), self.reverse().offset(distance))
		};

		if pos_offset.is_empty() || neg_offset.is_empty() {
			return Subpath::new(vec![], false);
		}

		pos_offset.combine_outline(&neg_offset, cap)
	}

	/// Version of the `outline` function which draws the outline at the specified distances away from the curve.
	/// The outline begins `start_distance` away, and gradually move to being `end_distance` away.
	/// <iframe frameBorder="0" width="100%" height="400px" src="https://graphite.rs/libraries/bezier-rs#bezier/graduated-outline/solo" title="Graduated Outline Demo"></iframe>
	pub fn graduated_outline<ManipulatorGroupId: crate::Identifier>(&self, start_distance: f64, end_distance: f64, cap: Cap) -> Subpath<ManipulatorGroupId> {
		self.skewed_outline(start_distance, end_distance, end_distance, start_distance, cap)
	}

	/// Version of the `graduated_outline` function that allows for the 4 corners of the outline to be different distances away from the curve.
	/// <iframe frameBorder="0" width="100%" height="475px" src="https://graphite.rs/libraries/bezier-rs#bezier/skewed-outline/solo" title="Skewed Outline Demo"></iframe>
	pub fn skewed_outline<ManipulatorGroupId: crate::Identifier>(&self, distance1: f64, distance2: f64, distance3: f64, distance4: f64, cap: Cap) -> Subpath<ManipulatorGroupId> {
		let (pos_offset, neg_offset) = if self.is_point() {
			(
				Subpath::new(vec![ManipulatorGroup::new_anchor(self.start() + DVec2::NEG_Y * distance1)], false),
				Subpath::new(vec![ManipulatorGroup::new_anchor(self.start() + DVec2::Y * distance1)], false),
			)
		} else {
			(self.graduated_offset(distance1, distance2), self.reverse().graduated_offset(distance3, distance4))
		};

		if pos_offset.is_empty() || neg_offset.is_empty() {
			return Subpath::new(vec![], false);
		}

		pos_offset.combine_outline(&neg_offset, cap)
	}

	/// Approximate a bezier curve with circular arcs.
	/// The algorithm can be customized using the [ArcsOptions] structure.
	/// <iframe frameBorder="0" width="100%" height="400px" src="https://graphite.rs/libraries/bezier-rs#bezier/arcs/solo" title="Arcs Demo"></iframe>
	pub fn arcs(&self, arcs_options: ArcsOptions) -> Vec<CircleArc> {
		let ArcsOptions {
			strategy: maximize_arcs,
			error,
			max_iterations,
		} = arcs_options;

		match maximize_arcs {
			ArcStrategy::Automatic => {
				let (auto_arcs, final_low_t) = self.approximate_curve_with_arcs(0., 1., error, max_iterations, true);
				let arc_approximations = self.split(TValue::Parametric(final_low_t))[1].arcs(ArcsOptions {
					strategy: ArcStrategy::FavorCorrectness,
					error,
					max_iterations,
				});
				if final_low_t != 1. {
					[auto_arcs, arc_approximations].concat()
				} else {
					auto_arcs
				}
			}
			ArcStrategy::FavorLargerArcs => self.approximate_curve_with_arcs(0., 1., error, max_iterations, false).0,
			ArcStrategy::FavorCorrectness => self
				.get_extrema_t_list()
				.windows(2)
				.flat_map(|t_pair| self.approximate_curve_with_arcs(t_pair[0], t_pair[1], error, max_iterations, false).0)
				.collect::<Vec<CircleArc>>(),
		}
	}

	/// Implements an algorithm that approximates a bezier curve with circular arcs.
	/// This algorithm uses a method akin to binary search to find an arc that approximates a maximal segment of the curve.
	/// Once a maximal arc has been found for a sub-segment of the curve, the algorithm continues by starting again at the end of the previous approximation.
	/// More details can be found in the [Approximating a Bezier curve with circular arcs](https://pomax.github.io/bezierinfo/#arcapproximation) section of Pomax's bezier curve primer.
	/// A caveat with this algorithm is that it is possible to find erroneous approximations in cases such as in a very narrow `U`.
	/// - `stop_when_invalid`: Used to determine whether the algorithm should terminate early if erroneous approximations are encountered.
	///
	/// Returns a tuple where the first element is the list of circular arcs and the second is the `t` value where the next segment should start from.
	/// The second value will be `1.` except for when `stop_when_invalid` is true and an invalid approximation is encountered.
	fn approximate_curve_with_arcs(&self, local_low: f64, local_high: f64, error: f64, max_iterations: usize, stop_when_invalid: bool) -> (Vec<CircleArc>, f64) {
		let mut low = local_low;
		let mut middle = (local_low + local_high) / 2.;
		let mut high = local_high;
		let mut previous_high = local_high;

		let mut iterations = 0;
		let mut previous_arc = CircleArc::default();
		let mut was_previous_good = false;
		let mut arcs = Vec::new();

		// Outer loop to iterate over the curve
		while low < local_high {
			// Inner loop to find the next maximal segment of the curve that can be approximated with a circular arc
			while iterations <= max_iterations {
				iterations += 1;
				let p1 = self.evaluate(TValue::Parametric(low));
				let p2 = self.evaluate(TValue::Parametric(middle));
				let p3 = self.evaluate(TValue::Parametric(high));

				let wrapped_center = utils::compute_circle_center_from_points(p1, p2, p3);
				// If the segment is linear, move on to next segment
				if wrapped_center.is_none() {
					previous_high = high;
					low = high;
					high = 1.;
					middle = (low + high) / 2.;
					was_previous_good = false;
					break;
				}

				let center = wrapped_center.unwrap();
				let radius = center.distance(p1);

				let angle_p1 = DVec2::new(1., 0.).angle_between(p1 - center);
				let angle_p2 = DVec2::new(1., 0.).angle_between(p2 - center);
				let angle_p3 = DVec2::new(1., 0.).angle_between(p3 - center);

				let mut start_angle = angle_p1;
				let mut end_angle = angle_p3;

				// Adjust start and end angles of the arc to ensure that it travels in the counter-clockwise direction
				if angle_p1 < angle_p3 {
					if angle_p2 < angle_p1 || angle_p3 < angle_p2 {
						std::mem::swap(&mut start_angle, &mut end_angle);
					}
				} else if angle_p2 < angle_p1 && angle_p3 < angle_p2 {
					std::mem::swap(&mut start_angle, &mut end_angle);
				}

				let new_arc = CircleArc {
					center,
					radius,
					start_angle,
					end_angle,
				};

				// Use points in between low, middle, and high to evaluate how well the arc approximates the curve
				let e1 = self.evaluate(TValue::Parametric((low + middle) / 2.));
				let e2 = self.evaluate(TValue::Parametric((middle + high) / 2.));

				// Iterate until we find the largest good approximation such that the next iteration is not a good approximation with an arc
				if utils::f64_compare(radius, e1.distance(center), error) && utils::f64_compare(radius, e2.distance(center), error) {
					// Check if the good approximation is actually valid: the sector angle cannot be larger than 180 degrees (PI radians)
					let mut sector_angle = end_angle - start_angle;
					if sector_angle < 0. {
						sector_angle += 2. * PI;
					}
					if stop_when_invalid && sector_angle > PI {
						return (arcs, low);
					}
					if high == local_high {
						// Found the final arc approximation
						arcs.push(new_arc);
						low = high;
						break;
					}
					// If the approximation is good, expand the segment by half to try finding a larger good approximation
					previous_high = high;
					high = (high + (high - low) / 2.).min(local_high);
					middle = (low + high) / 2.;
					previous_arc = new_arc;
					was_previous_good = true;
				} else if was_previous_good {
					// If the previous approximation was good and the current one is bad, then we use the previous good approximation
					arcs.push(previous_arc);

					// Continue searching for approximations for the rest of the curve
					low = previous_high;
					high = local_high;
					middle = low + (high - low) / 2.;
					was_previous_good = false;
					break;
				} else {
					// If no good approximation has been seen yet, try again with half the segment
					previous_high = high;
					high = middle;
					middle = low + (high - low) / 2.;
					previous_arc = new_arc;
				}
			}
		}

		(arcs, low)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::compare::{compare_arcs, compare_points};
	use crate::utils::{Cap, TValue};
	use crate::EmptyId;

	#[test]
	fn test_split() {
		let line = Bezier::from_linear_coordinates(25., 25., 75., 75.);
		let [part1, part2] = line.split(TValue::Parametric(0.5));

		assert_eq!(part1.start(), line.start());
		assert_eq!(part1.end(), line.evaluate(TValue::Parametric(0.5)));
		assert_eq!(part1.evaluate(TValue::Parametric(0.5)), line.evaluate(TValue::Parametric(0.25)));

		assert_eq!(part2.start(), line.evaluate(TValue::Parametric(0.5)));
		assert_eq!(part2.end(), line.end());
		assert_eq!(part2.evaluate(TValue::Parametric(0.5)), line.evaluate(TValue::Parametric(0.75)));

		let quad_bezier = Bezier::from_quadratic_coordinates(10., 10., 50., 50., 90., 10.);
		let [part3, part4] = quad_bezier.split(TValue::Parametric(0.5));

		assert_eq!(part3.start(), quad_bezier.start());
		assert_eq!(part3.end(), quad_bezier.evaluate(TValue::Parametric(0.5)));
		assert_eq!(part3.evaluate(TValue::Parametric(0.5)), quad_bezier.evaluate(TValue::Parametric(0.25)));

		assert_eq!(part4.start(), quad_bezier.evaluate(TValue::Parametric(0.5)));
		assert_eq!(part4.end(), quad_bezier.end());
		assert_eq!(part4.evaluate(TValue::Parametric(0.5)), quad_bezier.evaluate(TValue::Parametric(0.75)));

		let cubic_bezier = Bezier::from_cubic_coordinates(10., 10., 50., 50., 90., 10., 40., 50.);
		let [part5, part6] = cubic_bezier.split(TValue::Parametric(0.5));

		assert_eq!(part5.start(), cubic_bezier.start());
		assert_eq!(part5.end(), cubic_bezier.evaluate(TValue::Parametric(0.5)));
		assert_eq!(part5.evaluate(TValue::Parametric(0.5)), cubic_bezier.evaluate(TValue::Parametric(0.25)));

		assert_eq!(part6.start(), cubic_bezier.evaluate(TValue::Parametric(0.5)));
		assert_eq!(part6.end(), cubic_bezier.end());
		assert_eq!(part6.evaluate(TValue::Parametric(0.5)), cubic_bezier.evaluate(TValue::Parametric(0.75)));
	}

	#[test]
	fn test_split_at_anchors() {
		let start = DVec2::new(30., 50.);
		let end = DVec2::new(160., 170.);

		let bezier_quadratic = Bezier::from_quadratic_dvec2(start, DVec2::new(140., 30.), end);

		// Test splitting a quadratic bezier at the startpoint
		let [point_bezier1, remainder1] = bezier_quadratic.split(TValue::Parametric(0.));
		assert_eq!(point_bezier1, Bezier::from_quadratic_dvec2(start, start, start));
		assert!(remainder1.abs_diff_eq(&bezier_quadratic, MAX_ABSOLUTE_DIFFERENCE));

		// Test splitting a quadratic bezier at the endpoint
		let [remainder2, point_bezier2] = bezier_quadratic.split(TValue::Parametric(1.));
		assert_eq!(point_bezier2, Bezier::from_quadratic_dvec2(end, end, end));
		assert!(remainder2.abs_diff_eq(&bezier_quadratic, MAX_ABSOLUTE_DIFFERENCE));

		let bezier_cubic = Bezier::from_cubic_dvec2(start, DVec2::new(60., 140.), DVec2::new(150., 30.), end);

		// Test splitting a cubic bezier at the startpoint
		let [point_bezier3, remainder3] = bezier_cubic.split(TValue::Parametric(0.));
		assert_eq!(point_bezier3, Bezier::from_cubic_dvec2(start, start, start, start));
		assert!(remainder3.abs_diff_eq(&bezier_cubic, MAX_ABSOLUTE_DIFFERENCE));

		// Test splitting a cubic bezier at the endpoint
		let [remainder4, point_bezier4] = bezier_cubic.split(TValue::Parametric(1.));
		assert_eq!(point_bezier4, Bezier::from_cubic_dvec2(end, end, end, end));
		assert!(remainder4.abs_diff_eq(&bezier_cubic, MAX_ABSOLUTE_DIFFERENCE));
	}

	#[test]
	fn test_trim() {
		let line = Bezier::from_linear_coordinates(80., 80., 40., 40.);
		let trimmed1 = line.trim(TValue::Parametric(0.25), TValue::Parametric(0.75));

		assert_eq!(trimmed1.start(), line.evaluate(TValue::Parametric(0.25)));
		assert_eq!(trimmed1.end(), line.evaluate(TValue::Parametric(0.75)));
		assert_eq!(trimmed1.evaluate(TValue::Parametric(0.5)), line.evaluate(TValue::Parametric(0.5)));

		let quadratic_bezier = Bezier::from_quadratic_coordinates(80., 80., 40., 40., 70., 70.);
		let trimmed2 = quadratic_bezier.trim(TValue::Parametric(0.25), TValue::Parametric(0.75));

		assert_eq!(trimmed2.start(), quadratic_bezier.evaluate(TValue::Parametric(0.25)));
		assert_eq!(trimmed2.end(), quadratic_bezier.evaluate(TValue::Parametric(0.75)));
		assert_eq!(trimmed2.evaluate(TValue::Parametric(0.5)), quadratic_bezier.evaluate(TValue::Parametric(0.5)));

		let cubic_bezier = Bezier::from_cubic_coordinates(80., 80., 40., 40., 70., 70., 150., 150.);
		let trimmed3 = cubic_bezier.trim(TValue::Parametric(0.25), TValue::Parametric(0.75));

		assert!(trimmed3.start().abs_diff_eq(cubic_bezier.evaluate(TValue::Parametric(0.25)), MAX_ABSOLUTE_DIFFERENCE));
		assert_eq!(trimmed3.end(), cubic_bezier.evaluate(TValue::Parametric(0.75)));
		assert_eq!(trimmed3.evaluate(TValue::Parametric(0.5)), cubic_bezier.evaluate(TValue::Parametric(0.5)));
	}

	#[test]
	fn test_trim_t2_greater_than_t1() {
		// Test trimming quadratic curve when t2 > t1
		let bezier_quadratic = Bezier::from_quadratic_coordinates(30., 50., 140., 30., 160., 170.);
		let trim1 = bezier_quadratic.trim(TValue::Parametric(0.25), TValue::Parametric(0.75));
		let trim2 = bezier_quadratic.trim(TValue::Parametric(0.75), TValue::Parametric(0.25));
		assert!(trim1.abs_diff_eq(&trim2, MAX_ABSOLUTE_DIFFERENCE));

		// Test trimming cubic curve when t2 > t1
		let bezier_cubic = Bezier::from_cubic_coordinates(30., 30., 60., 140., 150., 30., 160., 160.);
		let trim3 = bezier_cubic.trim(TValue::Parametric(0.25), TValue::Parametric(0.75));
		let trim4 = bezier_cubic.trim(TValue::Parametric(0.75), TValue::Parametric(0.25));
		assert!(trim3.abs_diff_eq(&trim4, MAX_ABSOLUTE_DIFFERENCE));
	}

	#[test]
	fn test_rotate() {
		let bezier_linear = Bezier::from_linear_coordinates(30., 60., 140., 120.);
		let rotated_bezier_linear = bezier_linear.rotate(-PI / 2.);
		let expected_bezier_linear = Bezier::from_linear_coordinates(60., -30., 120., -140.);
		assert!(rotated_bezier_linear.abs_diff_eq(&expected_bezier_linear, MAX_ABSOLUTE_DIFFERENCE));

		let bezier_quadratic = Bezier::from_quadratic_coordinates(30., 50., 140., 30., 160., 170.);
		let rotated_bezier_quadratic = bezier_quadratic.rotate(PI);
		let expected_bezier_quadratic = Bezier::from_quadratic_coordinates(-30., -50., -140., -30., -160., -170.);
		assert!(rotated_bezier_quadratic.abs_diff_eq(&expected_bezier_quadratic, MAX_ABSOLUTE_DIFFERENCE));

		let bezier = Bezier::from_cubic_coordinates(30., 30., 60., 140., 150., 30., 160., 160.);
		let rotated_bezier = bezier.rotate(PI / 2.);
		let expected_bezier = Bezier::from_cubic_coordinates(-30., 30., -140., 60., -30., 150., -160., 160.);
		assert!(rotated_bezier.abs_diff_eq(&expected_bezier, MAX_ABSOLUTE_DIFFERENCE));
	}

	#[test]
	fn test_translate() {
		let bezier_linear = Bezier::from_linear_coordinates(30., 60., 140., 120.);
		let rotated_bezier_linear = bezier_linear.translate(DVec2::new(10., 10.));
		let expected_bezier_linear = Bezier::from_linear_coordinates(40., 70., 150., 130.);
		assert!(rotated_bezier_linear.abs_diff_eq(&expected_bezier_linear, MAX_ABSOLUTE_DIFFERENCE));

		let bezier_quadratic = Bezier::from_quadratic_coordinates(30., 50., 140., 30., 160., 170.);
		let rotated_bezier_quadratic = bezier_quadratic.translate(DVec2::new(-10., 10.));
		let expected_bezier_quadratic = Bezier::from_quadratic_coordinates(20., 60., 130., 40., 150., 180.);
		assert!(rotated_bezier_quadratic.abs_diff_eq(&expected_bezier_quadratic, MAX_ABSOLUTE_DIFFERENCE));

		let bezier = Bezier::from_cubic_coordinates(30., 30., 60., 140., 150., 30., 160., 160.);
		let translated_bezier = bezier.translate(DVec2::new(10., -10.));
		let expected_bezier = Bezier::from_cubic_coordinates(40., 20., 70., 130., 160., 20., 170., 150.);
		assert!(translated_bezier.abs_diff_eq(&expected_bezier, MAX_ABSOLUTE_DIFFERENCE));
	}

	#[test]
	fn test_reduce() {
		let p1 = DVec2::new(0., 0.);
		let p2 = DVec2::new(50., 50.);
		let p3 = DVec2::new(0., 0.);
		let bezier = Bezier::from_quadratic_dvec2(p1, p2, p3);

		let reduced_curves = bezier.reduce(None);
		assert!(reduced_curves.iter().all(|bezier| bezier.is_scalable()));

		// Check that the reduce helper is correct
		let (helper_curves, helper_t_values) = bezier.reduced_curves_and_t_values(None);
		assert!(reduced_curves
			.iter()
			.zip(helper_curves.iter())
			.all(|(bezier1, bezier2)| bezier1.abs_diff_eq(bezier2, MAX_ABSOLUTE_DIFFERENCE)));
		assert!(reduced_curves
			.iter()
			.zip(helper_t_values.iter())
			.all(|(curve, t_pair)| curve.abs_diff_eq(&bezier.trim(TValue::Parametric(t_pair[0]), TValue::Parametric(t_pair[1])), MAX_ABSOLUTE_DIFFERENCE)))
	}

	fn assert_valid_offset<ManipulatorGroupId: crate::Identifier>(bezier: &Bezier, offset: &Subpath<ManipulatorGroupId>, expected_distance: f64) {
		// Verify that the offset is smooth
		if offset.len() > 1 {
			offset.iter().take(offset.len() - 2).zip(offset.iter().skip(1)).for_each(|beziers_pair| {
				assert!(compare_points(beziers_pair.0.end, beziers_pair.1.start));
				assert!(compare_points(beziers_pair.0.normal(TValue::Parametric(1.)), beziers_pair.1.normal(TValue::Parametric(0.))));
			});
		}

		// Verify that the offset spans the length of the curve
		let start_distance = bezier.evaluate(TValue::Parametric(0.)).distance(offset.iter().next().unwrap().evaluate(TValue::Parametric(0.)));
		assert!(f64_compare(start_distance, expected_distance, MAX_ABSOLUTE_DIFFERENCE));
		let end_distance = bezier.evaluate(TValue::Parametric(1.)).distance(offset.iter().last().unwrap().evaluate(TValue::Parametric(1.)));
		assert!(f64_compare(end_distance, expected_distance, MAX_ABSOLUTE_DIFFERENCE));

		let err_threshold = expected_distance / 10.;
		// Sample the curve and verify that the offset lies at the correct distance from the curve.
		// Collect the t-value associated with the point on the bezier closest to the sample.
		let t_values: Vec<f64> = offset
			.iter()
			.flat_map(|offset_segment| {
				[0.1, 0.25, 0.5, 0.75, 0.9]
					.iter()
					.map(|t| {
						let offset_point = offset_segment.evaluate(TValue::Parametric(*t));
						let closest_point_t = bezier.project(offset_point, None);
						let closest_point = bezier.evaluate(TValue::Parametric(closest_point_t));
						let actual_distance = offset_point.distance(closest_point);

						assert!(f64_compare(actual_distance, expected_distance, err_threshold));
						closest_point_t
					})
					.collect::<Vec<f64>>()
			})
			.collect();

		// Verify that the curve segments are in the correct order by asserting that t_values is sorted
		for i in 1..t_values.len() {
			assert!(t_values[i - 1] < t_values[i]);
		}
	}

	#[test]
	fn test_offset_linear() {
		let start = DVec2::new(30., 60.);
		let end = DVec2::new(140., 120.);
		let bezier = Bezier::from_linear_dvec2(start, end);

		for distance in [-20., -10., 10., 20.] {
			let offset = bezier.offset::<EmptyId>(distance);
			assert_valid_offset(&bezier, &offset, distance.abs());
		}
	}

	#[test]
	fn test_offset_quadratic() {
		let start = DVec2::new(30., 50.);
		let handle = DVec2::new(140., 30.);
		let end = DVec2::new(160., 170.);
		let bezier = Bezier::from_quadratic_dvec2(start, handle, end);

		for distance in [-20., -10., 10., 20.] {
			let offset = bezier.offset::<EmptyId>(distance);
			assert_valid_offset(&bezier, &offset, distance.abs());
		}
	}

	#[test]
	fn test_offset_cubic() {
		let start = DVec2::new(30., 30.);
		let handle1 = DVec2::new(60., 140.);
		let handle2 = DVec2::new(150., 30.);
		let end = DVec2::new(160., 160.);
		let bezier = Bezier::from_cubic_dvec2(start, handle1, handle2, end);

		for distance in [-20., -10., 10., 20.] {
			let offset = bezier.offset::<EmptyId>(distance);
			assert_valid_offset(&bezier, &offset, distance.abs());
		}
	}

	#[test]
	fn test_offset_curve_that_has_a_single_point_after_reduce() {
		let p1 = DVec2::new(30., 30.);
		let p2 = DVec2::new(150., 29.);
		let p3 = DVec2::new(150., 30.);
		let p4 = DVec2::new(160., 160.);

		let bezier = Bezier::from_cubic_dvec2(p1, p2, p3, p4);

		let reduce = bezier.reduce(None);
		let offset = bezier.offset::<EmptyId>(15.);
		assert!(reduce.last().is_some());
		assert!(reduce.last().unwrap().is_point());
		// Expect the single point bezier to be dropped in the offset
		assert_eq!(reduce.len(), offset.len_segments() + 1);
	}

	#[test]
	fn test_outline() {
		let p1 = DVec2::new(30., 50.);
		let p2 = DVec2::new(140., 30.);
		let line = Bezier::from_linear_dvec2(p1, p2);
		let outline = line.outline::<EmptyId>(10., Cap::Butt);

		assert_eq!(outline.len(), 4);

		// Assert the first length-wise piece of the outline is 10 units from the line
		assert!(f64_compare(
			outline.iter().next().unwrap().evaluate(TValue::Parametric(0.25)).distance(line.evaluate(TValue::Parametric(0.25))),
			10.,
			MAX_ABSOLUTE_DIFFERENCE
		)); // f64

		// Assert the first cap touches the line end point at the halfway point
		assert!(outline.iter().nth(1).unwrap().evaluate(TValue::Parametric(0.5)).abs_diff_eq(line.end(), MAX_ABSOLUTE_DIFFERENCE));

		// Assert the second length-wise piece of the outline is 10 units from the line
		assert!(f64_compare(
			outline.iter().nth(2).unwrap().evaluate(TValue::Parametric(0.25)).distance(line.evaluate(TValue::Parametric(0.75))),
			10.,
			MAX_ABSOLUTE_DIFFERENCE
		)); // f64

		// Assert the second cap touches the line start point at the halfway point
		assert!(outline.iter().nth(3).unwrap().evaluate(TValue::Parametric(0.5)).abs_diff_eq(line.start(), MAX_ABSOLUTE_DIFFERENCE));
	}

	#[test]
	fn test_outline_single_point_circle() {
		let ellipse: Subpath<EmptyId> = Subpath::new_ellipse(DVec2::new(0., 0.), DVec2::new(50., 50.)).reverse();
		let p = DVec2::new(25., 25.);

		let line = Bezier::from_linear_dvec2(p, p);
		let outline = line.outline::<EmptyId>(25., Cap::Round);
		assert_eq!(outline, ellipse);

		let cubic = Bezier::from_cubic_dvec2(p, p, p, p);
		let outline_cubic = cubic.outline::<EmptyId>(25., Cap::Round);
		assert_eq!(outline_cubic, ellipse);
	}

	#[test]
	fn test_outline_single_point_square() {
		let square: Subpath<EmptyId> = Subpath::from_anchors(
			[
				DVec2::new(25., 0.),
				DVec2::new(0., 0.),
				DVec2::new(0., 50.),
				DVec2::new(25., 50.),
				DVec2::new(50., 50.),
				DVec2::new(50., 0.),
			],
			true,
		);
		let p = DVec2::new(25., 25.);

		let line = Bezier::from_linear_dvec2(p, p);
		let outline = line.outline::<EmptyId>(25., Cap::Square);
		assert_eq!(outline, square);

		let cubic = Bezier::from_cubic_dvec2(p, p, p, p);
		let outline_cubic = cubic.outline::<EmptyId>(25., Cap::Square);
		assert_eq!(outline_cubic, square);
	}

	#[test]
	fn test_graduated_scale() {
		let bezier = Bezier::from_linear_coordinates(30., 60., 140., 120.);
		bezier.graduated_scale(10., 20.);
	}

	#[test]
	fn test_graduated_scale_quadratic() {
		let bezier = Bezier::from_quadratic_coordinates(30., 50., 82., 98., 160., 170.);
		let scaled_bezier = bezier.graduated_scale(30., 30.);

		dbg!(scaled_bezier);

		// Assert the scaled bezier is 30 units from the line
		assert!(f64_compare(
			scaled_bezier.evaluate(TValue::Parametric(0.)).distance(bezier.evaluate(TValue::Parametric(0.))),
			30.,
			MAX_ABSOLUTE_DIFFERENCE
		));
		assert!(f64_compare(
			scaled_bezier.evaluate(TValue::Parametric(1.)).distance(bezier.evaluate(TValue::Parametric(1.))),
			30.,
			MAX_ABSOLUTE_DIFFERENCE
		));
		assert!(f64_compare(
			scaled_bezier.evaluate(TValue::Parametric(0.5)).distance(bezier.evaluate(TValue::Parametric(0.5))),
			30.,
			MAX_ABSOLUTE_DIFFERENCE
		));
	}

	#[test]
	fn test_arcs_linear() {
		let bezier = Bezier::from_linear_coordinates(30., 60., 140., 120.);
		let linear_arcs = bezier.arcs(ArcsOptions::default());
		assert!(linear_arcs.is_empty());
	}

	#[test]
	fn test_arcs_quadratic() {
		let bezier1 = Bezier::from_quadratic_coordinates(30., 30., 50., 50., 100., 100.);
		assert!(bezier1.arcs(ArcsOptions::default()).is_empty());

		let bezier2 = Bezier::from_quadratic_coordinates(50., 50., 85., 65., 100., 100.);
		let actual_arcs = bezier2.arcs(ArcsOptions::default());
		let expected_arc = CircleArc {
			center: DVec2::new(15., 135.),
			radius: 91.92388,
			start_angle: -1.18019,
			end_angle: -0.39061,
		};
		assert_eq!(actual_arcs.len(), 1);
		assert!(compare_arcs(actual_arcs[0], expected_arc));
	}

	#[test]
	fn test_arcs_cubic() {
		let bezier = Bezier::from_cubic_coordinates(30., 30., 30., 80., 60., 80., 60., 140.);
		let actual_arcs = bezier.arcs(ArcsOptions::default());
		let expected_arcs = [
			CircleArc {
				center: DVec2::new(122.394877, 30.7777189),
				radius: 92.39815,
				start_angle: 2.5637146,
				end_angle: -3.1331755,
			},
			CircleArc {
				center: DVec2::new(-47.54881, 136.169378),
				radius: 107.61701,
				start_angle: -0.53556,
				end_angle: 0.0356025,
			},
		];

		assert_eq!(actual_arcs.len(), 2);
		assert!(compare_arcs(actual_arcs[0], expected_arcs[0]));
		assert!(compare_arcs(actual_arcs[1], expected_arcs[1]));

		// Bezier that contains the erroneous case when maximizing arcs
		let bezier2 = Bezier::from_cubic_coordinates(48., 176., 170., 10., 30., 90., 180., 160.);
		let auto_arcs = bezier2.arcs(ArcsOptions::default());

		let extrema_arcs = bezier2.arcs(ArcsOptions {
			strategy: ArcStrategy::FavorCorrectness,
			..ArcsOptions::default()
		});

		let maximal_arcs = bezier2.arcs(ArcsOptions {
			strategy: ArcStrategy::FavorLargerArcs,
			..ArcsOptions::default()
		});

		// Resulting automatic arcs match the maximal results until the bad arc (in this case, only index 0 should match)
		assert_eq!(auto_arcs[0], maximal_arcs[0]);
		// Check that the first result from MaximizeArcs::Automatic should not equal the first results from MaximizeArcs::Off
		assert_ne!(auto_arcs[0], extrema_arcs[0]);
		// The remaining results (index 2 onwards) should match the results where MaximizeArcs::Off from the next extrema point onwards (after index 2).
		assert!(auto_arcs.iter().skip(2).zip(extrema_arcs.iter().skip(2)).all(|(arc1, arc2)| compare_arcs(*arc1, *arc2)));
	}
}
