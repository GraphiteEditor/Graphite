use super::style::{Fill, FillType, Gradient, GradientType, Stroke};
use super::{PointId, SegmentId, StrokeId, VectorData};
use crate::renderer::GraphicElementRendered;
use crate::transform::{Footprint, Transform, TransformMut};
use crate::{Color, GraphicGroup, Node};
use core::future::Future;

use bezier_rs::{Subpath, SubpathTValue, TValue};
use glam::{DAffine2, DVec2};
use rand::{Rng, SeedableRng};

#[derive(Debug, Clone, Copy)]
pub struct SetFillNode<FillType, SolidColor, GradientType, Start, End, Transform, Positions> {
	fill_type: FillType,
	solid_color: SolidColor,
	gradient_type: GradientType,
	start: Start,
	end: End,
	transform: Transform,
	positions: Positions,
}

#[node_macro::node_fn(SetFillNode)]
fn set_vector_data_fill(
	mut vector_data: VectorData,
	fill_type: FillType,
	solid_color: Option<Color>,
	gradient_type: GradientType,
	start: DVec2,
	end: DVec2,
	transform: DAffine2,
	positions: Vec<(f64, Color)>,
) -> VectorData {
	vector_data.style.set_fill(match fill_type {
		FillType::Solid => solid_color.map_or(Fill::None, Fill::Solid),
		FillType::Gradient => Fill::Gradient(Gradient {
			start,
			end,
			transform,
			positions,
			gradient_type,
		}),
	});
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
pub struct RepeatNode<Direction, Count> {
	direction: Direction,
	count: Count,
}

#[node_macro::node_fn(RepeatNode)]
fn repeat_vector_data(vector_data: VectorData, direction: DVec2, count: u32) -> VectorData {
	// Repeat the vector data
	let mut result = VectorData::empty();
	let inverse = vector_data.transform.inverse();
	let direction = inverse.transform_vector2(direction);
	for i in 0..count {
		let transform = DAffine2::from_translation(direction * i as f64);
		result.concat(&vector_data, transform);
	}

	result
}

#[derive(Debug, Clone, Copy)]
pub struct CircularRepeatNode<AngleOffset, Radius, Count> {
	angle_offset: AngleOffset,
	radius: Radius,
	count: Count,
}

#[node_macro::node_fn(CircularRepeatNode)]
fn circular_repeat_vector_data(vector_data: VectorData, angle_offset: f64, radius: f64, count: u32) -> VectorData {
	let mut result = VectorData::empty();

	let Some(bounding_box) = vector_data.bounding_box() else { return vector_data };
	let center = (bounding_box[0] + bounding_box[1]) / 2.;

	let base_transform = DVec2::new(0., radius) - center;

	for i in 0..count {
		let angle = (2. * std::f64::consts::PI / count as f64) * i as f64 + angle_offset.to_radians();
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
	let bounding_box = vector_data.bounding_box().unwrap();
	VectorData::from_subpath(Subpath::new_rect(
		vector_data.transform.transform_point2(bounding_box[0]),
		vector_data.transform.transform_point2(bounding_box[1]),
	))
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
async fn copy_to_points<I: GraphicElementRendered + Default + ConcatElement + TransformMut, FP: Future<Output = VectorData>, FI: Future<Output = I>>(
	footprint: Footprint,
	points: impl Node<Footprint, Output = FP>,
	instance: impl Node<Footprint, Output = FI>,
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
async fn sample_points<FV: Future<Output = VectorData>, FL: Future<Output = Vec<f64>>>(
	footprint: Footprint,
	mut vector_data: impl Node<Footprint, Output = FV>,
	spacing: f64,
	start_offset: f64,
	stop_offset: f64,
	adaptive_spacing: bool,
	lengths_of_segments_of_subpaths: impl Node<Footprint, Output = FL>,
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
	for (_, mut subpath) in vector_data.region_bezier_paths() {
		if subpath.manipulator_groups().len() < 3 {
			continue;
		}

		subpath.apply_transform(vector_data.transform);

		for point in subpath.poisson_disk_points(separation_disk_diameter, || rng.gen::<f64>()) {
			result.point_domain.push(PointId::generate(), vector_data.transform.inverse().transform_point2(point));
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

	for (start_index, end_index) in (0..(points.positions().len())).zip(1..(points.positions().len())) {
		let handle_start = first_handles[start_index];
		let handle_end = points.positions()[end_index] * 2. - first_handles[end_index];
		let handles = bezier_rs::BezierHandles::Cubic { handle_start, handle_end };

		vector_data
			.segment_domain
			.push(SegmentId::generate(), points.ids()[start_index], points.ids()[end_index], handles, StrokeId::generate())
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
async fn morph<SourceFuture: Future<Output = VectorData>, TargetFuture: Future<Output = VectorData>>(
	footprint: Footprint,
	source: impl Node<Footprint, Output = SourceFuture>,
	target: impl Node<Footprint, Output = TargetFuture>,
	start_index: u32,
	time: f64,
) -> VectorData {
	let source = self.source.eval(footprint).await;
	let target = self.target.eval(footprint).await;
	let mut result = VectorData::empty();

	// Lerp styles
	result.alpha_blending = if time < 0.5 { source.alpha_blending } else { target.alpha_blending };
	result.style = source.style.lerp(&target.style, time);

	let mut source_paths = source.stroke_bezier_paths();
	let mut target_paths = target.stroke_bezier_paths();
	for (mut source_path, mut target_path) in (&mut source_paths).zip(&mut target_paths) {
		// Deal with mistmatched transforms
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

		result.append_subpath(source_path);
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

#[cfg(test)]
mod test {
	use bezier_rs::Bezier;

	use super::*;
	use crate::transform::CullNode;
	use crate::value::ClonedNode;
	use std::pin::Pin;
	#[derive(Clone)]
	pub struct FutureWrapperNode<Node: Clone>(Node);

	impl<'i, T: 'i, N: Node<'i, T> + Clone> Node<'i, T> for FutureWrapperNode<N>
	where
		N: Node<'i, T>,
	{
		type Output = Pin<Box<dyn core::future::Future<Output = N::Output> + 'i>>;
		fn eval(&'i self, input: T) -> Self::Output {
			Box::pin(async move { self.0.eval(input) })
		}
	}

	#[test]
	fn repeat() {
		let direction = DVec2::X * 1.5;
		let repeated = RepeatNode {
			direction: ClonedNode::new(direction),
			count: ClonedNode::new(3),
		}
		.eval(VectorData::from_subpath(Subpath::new_rect(DVec2::ZERO, DVec2::ONE)));
		assert_eq!(repeated.region_bezier_paths().count(), 3);
		for (index, (_, subpath)) in repeated.region_bezier_paths().enumerate() {
			assert_eq!(subpath.manipulator_groups()[0].anchor, direction * index as f64);
		}
	}
	#[test]
	fn circle_repeat() {
		let repeated = CircularRepeatNode {
			angle_offset: ClonedNode::new(45.),
			radius: ClonedNode::new(4.),
			count: ClonedNode::new(8),
		}
		.eval(VectorData::from_subpath(Subpath::new_rect(DVec2::NEG_ONE, DVec2::ONE)));
		assert_eq!(repeated.region_bezier_paths().count(), 8);
		for (index, (_, subpath)) in repeated.region_bezier_paths().enumerate() {
			let expected_angle = (index as f64 + 1.) * 45.;
			let centre = (subpath.manipulator_groups()[0].anchor + subpath.manipulator_groups()[2].anchor) / 2.;
			let actual_angle = DVec2::Y.angle_between(centre).to_degrees();
			assert!((actual_angle - expected_angle).abs() % 360. < 1e-5);
		}
	}
	#[test]
	fn bounding_box() {
		let bouding_box = BoundingBoxNode.eval(VectorData::from_subpath(Subpath::new_rect(DVec2::NEG_ONE, DVec2::ONE)));
		assert_eq!(bouding_box.region_bezier_paths().count(), 1);
		let subpath = bouding_box.region_bezier_paths().next().unwrap().1;
		assert_eq!(&subpath.anchors()[..4], &[DVec2::NEG_ONE, DVec2::new(1., -1.), DVec2::ONE, DVec2::new(-1., 1.),]);
	}
	#[tokio::test]
	async fn copy_to_points() {
		let points = VectorData::from_subpath(Subpath::new_rect(DVec2::NEG_ONE * 10., DVec2::ONE * 10.));
		let expected_points = points.point_domain.positions().to_vec();
		let bouding_box = CopyToPoints {
			points: CullNode::new(FutureWrapperNode(ClonedNode(points))),
			instance: CullNode::new(FutureWrapperNode(ClonedNode(VectorData::from_subpath(Subpath::new_rect(DVec2::NEG_ONE, DVec2::ONE))))),
			random_scale_min: FutureWrapperNode(ClonedNode(1.)),
			random_scale_max: FutureWrapperNode(ClonedNode(1.)),
			random_scale_bias: FutureWrapperNode(ClonedNode(0.)),
			random_rotation: FutureWrapperNode(ClonedNode(0.)),
		}
		.eval(Footprint::default())
		.await;
		assert_eq!(bouding_box.region_bezier_paths().count(), expected_points.len());
		for (index, (_, subpath)) in bouding_box.region_bezier_paths().enumerate() {
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
