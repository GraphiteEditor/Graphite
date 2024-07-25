use super::misc::CentroidType;
use super::style::{Fill, Stroke};
use super::{PointId, SegmentId, StrokeId, VectorData};
use crate::renderer::GraphicElementRendered;
use crate::transform::{Footprint, Transform, TransformMut};
use crate::{Color, GraphicGroup, Node};

use bezier_rs::{Cap, Join, Subpath, SubpathTValue, TValue};
use glam::{DAffine2, DVec2};
use rand::{Rng, SeedableRng};

#[derive(Debug, Clone, Copy)]
pub struct SetFillNode<Fill> {
	fill: Fill,
}

#[node_macro::node_fn(SetFillNode)]
fn set_vector_data_fill<T: Into<Fill>>(mut vector_data: VectorData, fill: T) -> VectorData {
	vector_data.style.set_fill(fill.into());

	vector_data
}

#[derive(Debug, Clone, Copy)]
pub struct SetStrokeNode<Color, Weight, DashLengths, DashOffset, LineCap, LineJoin, MiterLimit> {
	color: Color,
	weight: Weight,
	dash_lengths: DashLengths,
	dash_offset: DashOffset,
	line_cap: LineCap,
	line_join: LineJoin,
	miter_limit: MiterLimit,
}

#[node_macro::node_fn(SetStrokeNode)]
fn set_vector_data_stroke(
	mut vector_data: VectorData,
	color: Option<Color>,
	weight: f64,
	dash_lengths: Vec<f64>,
	dash_offset: f64,
	line_cap: super::style::LineCap,
	line_join: super::style::LineJoin,
	miter_limit: f64,
) -> VectorData {
	vector_data.style.set_stroke(Stroke {
		color,
		weight,
		dash_lengths,
		dash_offset,
		line_cap,
		line_join,
		line_join_miter_limit: miter_limit,
	});
	vector_data
}

#[derive(Debug, Clone, Copy)]
pub struct RepeatNode<Direction, Angle, Instances> {
	direction: Direction,
	angle: Angle,
	instances: Instances,
}

#[node_macro::node_fn(RepeatNode)]
fn repeat_vector_data(vector_data: VectorData, direction: DVec2, angle: f64, instances: u32) -> VectorData {
	let angle = angle.to_radians();
	let instances = instances.max(1);
	let total = (instances - 1) as f64;

	if instances == 1 {
		return vector_data;
	}

	// Repeat the vector data
	let mut result = VectorData::empty();

	let Some(bounding_box) = vector_data.bounding_box_with_transform(vector_data.transform) else {
		return vector_data;
	};
	let center = (bounding_box[0] + bounding_box[1]) / 2.;

	for i in 0..instances {
		let translation = i as f64 * direction / total;
		let angle = i as f64 * angle / total;

		let transform = DAffine2::from_translation(center) * DAffine2::from_angle(angle) * DAffine2::from_translation(translation) * DAffine2::from_translation(-center);

		result.concat(&vector_data, transform);
	}

	result
}

#[derive(Debug, Clone, Copy)]
pub struct CircularRepeatNode<AngleOffset, Radius, Instances> {
	angle_offset: AngleOffset,
	radius: Radius,
	instances: Instances,
}

#[node_macro::node_fn(CircularRepeatNode)]
fn circular_repeat_vector_data(vector_data: VectorData, angle_offset: f64, radius: f64, instances: u32) -> VectorData {
	let instances = instances.max(1);

	if instances == 1 {
		return vector_data;
	}

	let mut result = VectorData::empty();

	let Some(bounding_box) = vector_data.bounding_box_with_transform(vector_data.transform) else {
		return vector_data;
	};
	let center = (bounding_box[0] + bounding_box[1]) / 2.;

	let base_transform = DVec2::new(0., radius) - center;

	for i in 0..instances {
		let angle = (std::f64::consts::TAU / instances as f64) * i as f64 + angle_offset.to_radians();
		let rotation = DAffine2::from_angle(angle);
		let transform = DAffine2::from_translation(center) * rotation * DAffine2::from_translation(base_transform);
		result.concat(&vector_data, transform);
	}

	result
}

#[derive(Debug, Clone, Copy)]
pub struct BoundingBoxNode;

#[node_macro::node_fn(BoundingBoxNode)]
fn generate_bounding_box(vector_data: VectorData) -> VectorData {
	let bounding_box = vector_data.bounding_box_with_transform(vector_data.transform).unwrap();
	VectorData::from_subpath(Subpath::new_rect(bounding_box[0], bounding_box[1]))
}

#[derive(Debug, Clone, Copy)]
pub struct SolidifyStrokeNode;

#[node_macro::node_fn(SolidifyStrokeNode)]
fn solidify_stroke(vector_data: VectorData) -> VectorData {
	// Grab what we need from original data.
	let VectorData { transform, style, .. } = &vector_data;
	let subpaths = vector_data.stroke_bezier_paths();
	let mut result = VectorData::empty();

	// Perform operation on all subpaths in this shape.
	for mut subpath in subpaths {
		let stroke = style.stroke().unwrap();
		subpath.apply_transform(*transform);

		// Taking the existing stroke data and passing it to Bezier-rs to generate new paths.
		let subpath_out = subpath.outline(
			stroke.weight / 2., // Diameter to radius.
			match stroke.line_join {
				crate::vector::style::LineJoin::Miter => Join::Miter(Some(stroke.line_join_miter_limit)),
				crate::vector::style::LineJoin::Bevel => Join::Bevel,
				crate::vector::style::LineJoin::Round => Join::Round,
			},
			match stroke.line_cap {
				crate::vector::style::LineCap::Butt => Cap::Butt,
				crate::vector::style::LineCap::Round => Cap::Round,
				crate::vector::style::LineCap::Square => Cap::Square,
			},
		);

		// This is where we determine whether we have a closed or open path. Ex: Oval vs line segment.
		if subpath_out.1.is_some() {
			// Two closed subpaths, closed shape. Add both subpaths.
			result.append_subpath(subpath_out.0, false);
			result.append_subpath(subpath_out.1.unwrap(), false);
		} else {
			// One closed subpath, open path.
			result.append_subpath(subpath_out.0, false);
		}
	}

	// We set our fill to our stroke's color, then clear our stroke.
	if let Some(stroke) = vector_data.style.stroke() {
		result.style.set_fill(Fill::solid_or_none(stroke.color));
		result.style.set_stroke(Stroke::default());
	}

	result
}

pub trait ConcatElement {
	fn concat(&mut self, other: &Self, transform: DAffine2);
}

impl ConcatElement for GraphicGroup {
	fn concat(&mut self, other: &Self, transform: DAffine2) {
		// TODO: Decide if we want to keep this behavior whereby the layers are flattened
		for mut element in other.iter().cloned() {
			*element.transform_mut() = transform * element.transform() * other.transform();
			self.push(element);
		}
		self.alpha_blending = other.alpha_blending;
	}
}

#[derive(Debug, Clone, Copy)]
pub struct CopyToPoints<Points, Instance, RandomScaleMin, RandomScaleMax, RandomScaleBias, RandomRotation> {
	points: Points,
	instance: Instance,
	random_scale_min: RandomScaleMin,
	random_scale_max: RandomScaleMax,
	random_scale_bias: RandomScaleBias,
	random_rotation: RandomRotation,
}

#[node_macro::node_fn(CopyToPoints)]
async fn copy_to_points<I: GraphicElementRendered + Default + ConcatElement + TransformMut + Send>(
	footprint: Footprint,
	points: impl Node<Footprint, Output = VectorData>,
	instance: impl Node<Footprint, Output = I>,
	random_scale_min: f64,
	random_scale_max: f64,
	random_scale_bias: f64,
	random_rotation: f64,
) -> I {
	let points = self.points.eval(footprint).await;
	let instance = self.instance.eval(footprint).await;
	let random_scale_difference = random_scale_max - random_scale_min;

	let points_list = points.point_domain.positions();

	let instance_bounding_box = instance.bounding_box(DAffine2::IDENTITY).unwrap_or_default();
	let instance_center = -0.5 * (instance_bounding_box[0] + instance_bounding_box[1]);

	let mut scale_rng = rand::rngs::StdRng::seed_from_u64(0);
	let mut rotation_rng = rand::rngs::StdRng::seed_from_u64(0);

	let do_scale = random_scale_difference.abs() > 1e-6;
	let do_rotation = random_rotation.abs() > 1e-6;

	let mut result = I::default();
	for &point in points_list {
		let center_transform = DAffine2::from_translation(instance_center);

		let translation = points.transform.transform_point2(point);

		let rotation = if do_rotation {
			let degrees = (rotation_rng.gen::<f64>() - 0.5) * random_rotation;
			degrees / 360. * std::f64::consts::TAU
		} else {
			0.
		};

		let scale = if do_scale {
			if random_scale_bias.abs() < 1e-6 {
				// Linear
				random_scale_min + scale_rng.gen::<f64>() * random_scale_difference
			} else {
				// Weighted (see <https://www.desmos.com/calculator/gmavd3m9bd>)
				let horizontal_scale_factor = 1. - 2_f64.powf(random_scale_bias);
				let scale_factor = (1. - scale_rng.gen::<f64>() * horizontal_scale_factor).log2() / random_scale_bias;
				random_scale_min + scale_factor * random_scale_difference
			}
		} else {
			random_scale_min
		};

		result.concat(&instance, DAffine2::from_scale_angle_translation(DVec2::splat(scale), rotation, translation) * center_transform);
	}

	result
}

#[derive(Debug, Clone, Copy)]
pub struct SamplePoints<VectorData, Spacing, StartOffset, StopOffset, AdaptiveSpacing, LengthsOfSegmentsOfSubpaths> {
	vector_data: VectorData,
	spacing: Spacing,
	start_offset: StartOffset,
	stop_offset: StopOffset,
	adaptive_spacing: AdaptiveSpacing,
	lengths_of_segments_of_subpaths: LengthsOfSegmentsOfSubpaths,
}

#[node_macro::node_fn(SamplePoints)]
async fn sample_points(
	footprint: Footprint,
	mut vector_data: impl Node<Footprint, Output = VectorData>,
	spacing: f64,
	start_offset: f64,
	stop_offset: f64,
	adaptive_spacing: bool,
	lengths_of_segments_of_subpaths: impl Node<Footprint, Output = Vec<f64>>,
) -> VectorData {
	let vector_data = self.vector_data.eval(footprint).await;
	let lengths_of_segments_of_subpaths = self.lengths_of_segments_of_subpaths.eval(footprint).await;

	let mut bezier = vector_data.segment_bezier_iter().enumerate().peekable();

	let mut result = VectorData::empty();
	result.transform = vector_data.transform;

	while let Some((index, (segment, _, _, mut last_end))) = bezier.next() {
		let mut lengths = vec![(segment, lengths_of_segments_of_subpaths.get(index).copied().unwrap_or_default())];

		while let Some((index, (segment, _, _, end))) = bezier.peek().is_some_and(|(_, (_, _, start, _))| *start == last_end).then(|| bezier.next()).flatten() {
			last_end = end;
			lengths.push((segment, lengths_of_segments_of_subpaths.get(index).copied().unwrap_or_default()));
		}

		let total_length: f64 = lengths.iter().map(|(_, len)| *len).sum();

		let mut used_length = total_length - start_offset - stop_offset;
		if used_length <= 0. {
			continue;
		}

		let count;
		if adaptive_spacing {
			// With adaptive spacing, we widen or narrow the points as necessary to ensure the last point is always at the end of the path.
			count = (used_length / spacing).round();
		} else {
			// Without adaptive spacing, we just evenly space the points at the exact specified spacing, usually falling short before the end of the path.
			count = (used_length / spacing + f64::EPSILON).floor();
			used_length = used_length - used_length % spacing;
		}

		if count < 1. {
			continue;
		}
		for c in 0..=count as usize {
			let fraction = c as f64 / count;
			let total_distance = fraction * used_length + start_offset;

			let (mut segment, mut length) = lengths[0];
			let mut total_length_before = 0.;
			for &(next_segment, next_length) in lengths.iter().skip(1) {
				if total_length_before + length > total_distance {
					break;
				}

				total_length_before += length;
				segment = next_segment;
				length = next_length;
			}

			let Some(segment) = vector_data.segment_from_id(segment) else { continue };
			let segment = segment.apply_transformation(|point| vector_data.transform.transform_point2(point));

			let parametric_t = segment.euclidean_to_parametric_with_total_length((total_distance - total_length_before) / length, 0.001, length);
			let point = segment.evaluate(TValue::Parametric(parametric_t));
			result.point_domain.push(PointId::generate(), vector_data.transform.inverse().transform_point2(point));
		}
	}

	result
}

#[derive(Debug, Clone, Copy)]
pub struct PoissonDiskPoints<SeparationDiskDiameter> {
	separation_disk_diameter: SeparationDiskDiameter,
}

#[node_macro::node_fn(PoissonDiskPoints)]
fn poisson_disk_points(vector_data: VectorData, separation_disk_diameter: f64) -> VectorData {
	let mut rng = rand::rngs::StdRng::seed_from_u64(0);
	let mut result = VectorData::empty();
	for mut subpath in vector_data.stroke_bezier_paths() {
		if subpath.manipulator_groups().len() < 3 {
			continue;
		}

		subpath.apply_transform(vector_data.transform);

		for point in subpath.poisson_disk_points(separation_disk_diameter, || rng.gen::<f64>()) {
			result.point_domain.push(PointId::generate(), point);
		}
	}

	result
}

#[derive(Debug, Clone, Copy)]
pub struct LengthsOfSegmentsOfSubpaths;

#[node_macro::node_fn(LengthsOfSegmentsOfSubpaths)]
fn lengths_of_segments_of_subpaths(vector_data: VectorData) -> Vec<f64> {
	vector_data
		.segment_bezier_iter()
		.map(|(_id, bezier, _, _)| bezier.apply_transformation(|point| vector_data.transform.transform_point2(point)).length(None))
		.collect()
}

#[derive(Debug, Clone, Copy)]
pub struct SplinesFromPointsNode;

#[node_macro::node_fn(SplinesFromPointsNode)]
fn splines_from_points(mut vector_data: VectorData) -> VectorData {
	let points = &vector_data.point_domain;

	vector_data.segment_domain.clear();

	let first_handles = bezier_rs::solve_spline_first_handle(points.positions());

	let stroke_id = StrokeId::ZERO;

	for (start_index, end_index) in (0..(points.positions().len())).zip(1..(points.positions().len())) {
		let handle_start = first_handles[start_index];
		let handle_end = points.positions()[end_index] * 2. - first_handles[end_index];
		let handles = bezier_rs::BezierHandles::Cubic { handle_start, handle_end };

		vector_data
			.segment_domain
			.push(SegmentId::generate(), points.ids()[start_index], points.ids()[end_index], handles, stroke_id)
	}

	vector_data
}

pub struct MorphNode<Source, Target, StartIndex, Time> {
	source: Source,
	target: Target,
	start_index: StartIndex,
	time: Time,
}

#[node_macro::node_fn(MorphNode)]
async fn morph(footprint: Footprint, source: impl Node<Footprint, Output = VectorData>, target: impl Node<Footprint, Output = VectorData>, start_index: u32, time: f64) -> VectorData {
	let source = self.source.eval(footprint).await;
	let target = self.target.eval(footprint).await;
	let mut result = VectorData::empty();

	// Lerp styles
	result.alpha_blending = if time < 0.5 { source.alpha_blending } else { target.alpha_blending };
	result.style = source.style.lerp(&target.style, time);

	let mut source_paths = source.stroke_bezier_paths();
	let mut target_paths = target.stroke_bezier_paths();
	for (mut source_path, mut target_path) in (&mut source_paths).zip(&mut target_paths) {
		// Deal with mismatched transforms
		source_path.apply_transform(source.transform);
		target_path.apply_transform(target.transform);

		// Deal with mismatched start index
		for _ in 0..start_index {
			let first = target_path.remove_manipulator_group(0);
			target_path.push_manipulator_group(first);
		}

		// Deal with mismatched closed state
		if source_path.closed() && !target_path.closed() {
			source_path.set_closed(false);
			source_path.push_manipulator_group(source_path.manipulator_groups()[0].flip());
		}
		if !source_path.closed() && target_path.closed() {
			target_path.set_closed(false);
			target_path.push_manipulator_group(target_path.manipulator_groups()[0].flip());
		}

		// Mismatched subpath items
		'outer: loop {
			for segment_index in (0..(source_path.len() - 1)).rev() {
				if target_path.len() <= source_path.len() {
					break 'outer;
				}
				source_path.insert(SubpathTValue::Parametric { segment_index, t: 0.5 })
			}
		}
		'outer: loop {
			for segment_index in (0..(target_path.len() - 1)).rev() {
				if source_path.len() <= target_path.len() {
					break 'outer;
				}
				target_path.insert(SubpathTValue::Parametric { segment_index, t: 0.5 })
			}
		}

		// Lerp points
		for (manipulator, target) in source_path.manipulator_groups_mut().iter_mut().zip(target_path.manipulator_groups()) {
			manipulator.in_handle = Some(manipulator.in_handle.unwrap_or(manipulator.anchor).lerp(target.in_handle.unwrap_or(target.anchor), time));
			manipulator.out_handle = Some(manipulator.out_handle.unwrap_or(manipulator.anchor).lerp(target.out_handle.unwrap_or(target.anchor), time));
			manipulator.anchor = manipulator.anchor.lerp(target.anchor, time);
		}

		result.append_subpath(source_path, true);
	}
	// Mismatched subpath count
	for mut source_path in source_paths {
		source_path.apply_transform(source.transform);
		let end = source_path.manipulator_groups().first().map(|group| group.anchor).unwrap_or_default();
		for group in source_path.manipulator_groups_mut() {
			group.anchor = group.anchor.lerp(end, time);
			group.in_handle = group.in_handle.map(|handle| handle.lerp(end, time));
			group.out_handle = group.in_handle.map(|handle| handle.lerp(end, time));
		}
	}
	for mut target_path in target_paths {
		target_path.apply_transform(target.transform);
		let start = target_path.manipulator_groups().first().map(|group| group.anchor).unwrap_or_default();
		for group in target_path.manipulator_groups_mut() {
			group.anchor = start.lerp(group.anchor, time);
			group.in_handle = group.in_handle.map(|handle| start.lerp(handle, time));
			group.out_handle = group.in_handle.map(|handle| start.lerp(handle, time));
		}
	}

	result
}

#[derive(Debug, Clone, Copy)]
pub struct AreaNode<VectorData> {
	vector_data: VectorData,
}

#[node_macro::node_fn(AreaNode)]
async fn area_node(empty: (), vector_data: impl Node<Footprint, Output = VectorData>) -> f64 {
	let vector_data = self.vector_data.eval(Footprint::default()).await;

	let mut area = 0.;
	let scale = vector_data.transform.decompose_scale();
	for subpath in vector_data.stroke_bezier_paths() {
		area += subpath.area(Some(1e-3), Some(1e-3));
	}
	area * scale[0] * scale[1]
}

#[derive(Debug, Clone, Copy)]
pub struct CentroidNode<VectorData, CentroidType> {
	vector_data: VectorData,
	centroid_type: CentroidType,
}

#[node_macro::node_fn(CentroidNode)]
async fn centroid_node(empty: (), vector_data: impl Node<Footprint, Output = VectorData>, centroid_type: CentroidType) -> DVec2 {
	let vector_data = self.vector_data.eval(Footprint::default()).await;

	if centroid_type == CentroidType::Area {
		let mut area = 0.;
		let mut centroid = DVec2::ZERO;
		for subpath in vector_data.stroke_bezier_paths() {
			if let Some((subpath_centroid, subpath_area)) = subpath.area_centroid_and_area(Some(1e-3), Some(1e-3)) {
				if subpath_area == 0. {
					continue;
				}
				area += subpath_area;
				centroid += subpath_area * subpath_centroid;
			}
		}

		if area != 0. {
			centroid /= area;
			return vector_data.transform().transform_point2(centroid);
		}
	}

	let mut length = 0.;
	let mut centroid = DVec2::ZERO;
	for subpath in vector_data.stroke_bezier_paths() {
		if let Some((subpath_centroid, subpath_length)) = subpath.length_centroid_and_length(None, true) {
			length += subpath_length;
			centroid += subpath_length * subpath_centroid;
		}
	}

	if length != 0. {
		centroid /= length;
		return vector_data.transform().transform_point2(centroid);
	}

	let positions = vector_data.point_domain.positions();
	if !positions.is_empty() {
		let centroid = positions.iter().sum::<DVec2>() / (positions.len() as f64);
		return vector_data.transform().transform_point2(centroid);
	}

	DVec2::ZERO
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::transform::CullNode;
	use crate::value::ClonedNode;

	use bezier_rs::Bezier;

	use std::pin::Pin;

	#[derive(Clone)]
	pub struct FutureWrapperNode<Node: Clone>(Node);

	impl<'i, T: 'i, N: Node<'i, T> + Clone> Node<'i, T> for FutureWrapperNode<N>
	where
		N: Node<'i, T, Output: Send>,
	{
		type Output = Pin<Box<dyn core::future::Future<Output = N::Output> + 'i + Send>>;
		fn eval(&'i self, input: T) -> Self::Output {
			let result = self.0.eval(input);
			Box::pin(async move { result })
		}
	}

	#[test]
	fn repeat() {
		let direction = DVec2::X * 1.5;
		let instances = 3;
		let repeated = RepeatNode {
			direction: ClonedNode::new(direction),
			angle: ClonedNode::new(0.),
			instances: ClonedNode::new(instances),
		}
		.eval(VectorData::from_subpath(Subpath::new_rect(DVec2::ZERO, DVec2::ONE)));
		assert_eq!(repeated.region_bezier_paths().count(), 3);
		for (index, (_, subpath)) in repeated.region_bezier_paths().enumerate() {
			assert!((subpath.manipulator_groups()[0].anchor - direction * index as f64 / (instances - 1) as f64).length() < 1e-5);
		}
	}
	#[test]
	fn repeat_transform_position() {
		let direction = DVec2::new(12., 10.);
		let instances = 8;
		let repeated = RepeatNode {
			direction: ClonedNode::new(direction),
			angle: ClonedNode::new(0.),
			instances: ClonedNode::new(instances),
		}
		.eval(VectorData::from_subpath(Subpath::new_rect(DVec2::ZERO, DVec2::ONE)));
		assert_eq!(repeated.region_bezier_paths().count(), 8);
		for (index, (_, subpath)) in repeated.region_bezier_paths().enumerate() {
			assert!((subpath.manipulator_groups()[0].anchor - direction * index as f64 / (instances - 1) as f64).length() < 1e-5);
		}
	}
	#[test]
	fn circle_repeat() {
		let repeated = CircularRepeatNode {
			angle_offset: ClonedNode::new(45.),
			radius: ClonedNode::new(4.),
			instances: ClonedNode::new(8),
		}
		.eval(VectorData::from_subpath(Subpath::new_rect(DVec2::NEG_ONE, DVec2::ONE)));
		assert_eq!(repeated.region_bezier_paths().count(), 8);
		for (index, (_, subpath)) in repeated.region_bezier_paths().enumerate() {
			let expected_angle = (index as f64 + 1.) * 45.;
			let center = (subpath.manipulator_groups()[0].anchor + subpath.manipulator_groups()[2].anchor) / 2.;
			let actual_angle = DVec2::Y.angle_to(center).to_degrees();
			assert!((actual_angle - expected_angle).abs() % 360. < 1e-5);
		}
	}
	#[test]
	fn bounding_box() {
		let bounding_box = BoundingBoxNode.eval(VectorData::from_subpath(Subpath::new_rect(DVec2::NEG_ONE, DVec2::ONE)));
		assert_eq!(bounding_box.region_bezier_paths().count(), 1);
		let subpath = bounding_box.region_bezier_paths().next().unwrap().1;
		assert_eq!(&subpath.anchors()[..4], &[DVec2::NEG_ONE, DVec2::new(1., -1.), DVec2::ONE, DVec2::new(-1., 1.),]);

		// test a VectorData with non-zero rotation
		let mut square = VectorData::from_subpath(Subpath::new_rect(DVec2::NEG_ONE, DVec2::ONE));
		square.transform *= DAffine2::from_angle(core::f64::consts::FRAC_PI_4);
		let bounding_box = BoundingBoxNode.eval(square);
		assert_eq!(bounding_box.region_bezier_paths().count(), 1);
		let subpath = bounding_box.region_bezier_paths().next().unwrap().1;
		let sqrt2 = core::f64::consts::SQRT_2;
		let sqrt2_bounding_box = [DVec2::new(-sqrt2, -sqrt2), DVec2::new(sqrt2, -sqrt2), DVec2::new(sqrt2, sqrt2), DVec2::new(-sqrt2, sqrt2)];
		assert!(subpath.anchors()[..4].iter().zip(sqrt2_bounding_box).all(|(p1, p2)| p1.abs_diff_eq(p2, f64::EPSILON)));
	}
	#[tokio::test]
	async fn copy_to_points() {
		let points = VectorData::from_subpath(Subpath::new_rect(DVec2::NEG_ONE * 10., DVec2::ONE * 10.));
		let expected_points = points.point_domain.positions().to_vec();
		let bounding_box = CopyToPoints {
			points: CullNode::new(FutureWrapperNode(ClonedNode(points))),
			instance: CullNode::new(FutureWrapperNode(ClonedNode(VectorData::from_subpath(Subpath::new_rect(DVec2::NEG_ONE, DVec2::ONE))))),
			random_scale_min: FutureWrapperNode(ClonedNode(1.)),
			random_scale_max: FutureWrapperNode(ClonedNode(1.)),
			random_scale_bias: FutureWrapperNode(ClonedNode(0.)),
			random_rotation: FutureWrapperNode(ClonedNode(0.)),
		}
		.eval(Footprint::default())
		.await;
		assert_eq!(bounding_box.region_bezier_paths().count(), expected_points.len());
		for (index, (_, subpath)) in bounding_box.region_bezier_paths().enumerate() {
			let offset = expected_points[index];
			assert_eq!(
				&subpath.anchors()[..4],
				&[offset + DVec2::NEG_ONE, offset + DVec2::new(1., -1.), offset + DVec2::ONE, offset + DVec2::new(-1., 1.),]
			);
		}
	}
	#[tokio::test]
	async fn sample_points() {
		let path = VectorData::from_subpath(Subpath::from_bezier(&Bezier::from_cubic_dvec2(DVec2::ZERO, DVec2::ZERO, DVec2::X * 100., DVec2::X * 100.)));
		let sample_points = SamplePoints {
			vector_data: CullNode::new(FutureWrapperNode(ClonedNode(path))),
			spacing: FutureWrapperNode(ClonedNode(30.)),
			start_offset: FutureWrapperNode(ClonedNode(0.)),
			stop_offset: FutureWrapperNode(ClonedNode(0.)),
			adaptive_spacing: FutureWrapperNode(ClonedNode(false)),
			lengths_of_segments_of_subpaths: CullNode::new(FutureWrapperNode(ClonedNode(vec![100.]))),
		}
		.eval(Footprint::default())
		.await;
		assert_eq!(sample_points.point_domain.positions().len(), 4);
		for (pos, expected) in sample_points.point_domain.positions().iter().zip([DVec2::X * 0., DVec2::X * 30., DVec2::X * 60., DVec2::X * 90.]) {
			assert!(pos.distance(expected) < 1e-3, "Expected {expected} found {pos}");
		}
	}
	#[tokio::test]
	async fn adaptive_spacing() {
		let path = VectorData::from_subpath(Subpath::from_bezier(&Bezier::from_cubic_dvec2(DVec2::ZERO, DVec2::ZERO, DVec2::X * 100., DVec2::X * 100.)));
		let sample_points = SamplePoints {
			vector_data: CullNode::new(FutureWrapperNode(ClonedNode(path))),
			spacing: FutureWrapperNode(ClonedNode(18.)),
			start_offset: FutureWrapperNode(ClonedNode(45.)),
			stop_offset: FutureWrapperNode(ClonedNode(10.)),
			adaptive_spacing: FutureWrapperNode(ClonedNode(true)),
			lengths_of_segments_of_subpaths: CullNode::new(FutureWrapperNode(ClonedNode(vec![100.]))),
		}
		.eval(Footprint::default())
		.await;
		assert_eq!(sample_points.point_domain.positions().len(), 4);
		for (pos, expected) in sample_points.point_domain.positions().iter().zip([DVec2::X * 45., DVec2::X * 60., DVec2::X * 75., DVec2::X * 90.]) {
			assert!(pos.distance(expected) < 1e-3, "Expected {expected} found {pos}");
		}
	}
	#[test]
	fn poisson() {
		let sample_points = PoissonDiskPoints {
			separation_disk_diameter: ClonedNode(10. * std::f64::consts::SQRT_2),
		}
		.eval(VectorData::from_subpath(Subpath::new_ellipse(DVec2::NEG_ONE * 50., DVec2::ONE * 50.)));
		assert!(
			(20..=40).contains(&sample_points.point_domain.positions().len()),
			"actual len {}",
			sample_points.point_domain.positions().len()
		);
		for point in sample_points.point_domain.positions() {
			assert!(point.length() < 50. + 1., "Expected point in circle {point}")
		}
	}
	#[test]
	fn lengths() {
		let subpath = VectorData::from_subpath(Subpath::from_bezier(&Bezier::from_cubic_dvec2(DVec2::ZERO, DVec2::ZERO, DVec2::X * 100., DVec2::X * 100.)));
		let lengths = LengthsOfSegmentsOfSubpaths.eval(subpath);
		assert_eq!(lengths, vec![100.]);
	}
	#[test]
	fn spline() {
		let subpath = VectorData::from_subpath(Subpath::new_rect(DVec2::ZERO, DVec2::ONE * 100.));
		let spline = SplinesFromPointsNode.eval(subpath);
		assert_eq!(spline.stroke_bezier_paths().count(), 1);
		assert_eq!(spline.point_domain.positions(), &[DVec2::ZERO, DVec2::new(100., 0.), DVec2::new(100., 100.), DVec2::new(0., 100.)]);
	}
	#[tokio::test]
	async fn morph() {
		let source = VectorData::from_subpath(Subpath::new_rect(DVec2::ZERO, DVec2::ONE * 100.));
		let target = VectorData::from_subpath(Subpath::new_ellipse(DVec2::NEG_ONE * 100., DVec2::ZERO));
		let sample_points = MorphNode {
			source: CullNode::new(FutureWrapperNode(ClonedNode(source))),
			target: CullNode::new(FutureWrapperNode(ClonedNode(target))),
			time: FutureWrapperNode(ClonedNode(0.5)),
			start_index: FutureWrapperNode(ClonedNode(0)),
		}
		.eval(Footprint::default())
		.await;
		assert_eq!(
			&sample_points.point_domain.positions()[..4],
			vec![DVec2::new(-25., -50.), DVec2::new(50., -25.), DVec2::new(25., 50.), DVec2::new(-50., 25.)]
		);
	}
}
