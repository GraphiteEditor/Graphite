use super::style::{Fill, FillType, Gradient, GradientType, Stroke};
use super::VectorData;
use crate::{Color, Node};

use bezier_rs::{Subpath, SubpathTValue};
use glam::{DAffine2, DVec2};

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
	positions: Vec<(f64, Option<Color>)>,
) -> VectorData {
	vector_data.style.set_fill(match fill_type {
		FillType::None | FillType::Solid => solid_color.map_or(Fill::None, Fill::Solid),
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
	weight: f32,
	dash_lengths: Vec<f32>,
	dash_offset: f32,
	line_cap: super::style::LineCap,
	line_join: super::style::LineJoin,
	miter_limit: f32,
) -> VectorData {
	vector_data.style.set_stroke(Stroke {
		color,
		weight: weight as f64,
		dash_lengths,
		dash_offset: dash_offset as f64,
		line_cap,
		line_join,
		line_join_miter_limit: miter_limit as f64,
	});
	vector_data
}

#[derive(Debug, Clone, Copy)]
pub struct RepeatNode<Direction, Count> {
	direction: Direction,
	count: Count,
}

#[node_macro::node_fn(RepeatNode)]
fn repeat_vector_data(mut vector_data: VectorData, direction: DVec2, count: u32) -> VectorData {
	// repeat the vector data
	let VectorData { subpaths, transform, .. } = &vector_data;

	let mut new_subpaths: Vec<Subpath<_>> = Vec::with_capacity(subpaths.len() * count as usize);
	let inverse = transform.inverse();
	let direction = inverse.transform_vector2(direction);
	for i in 0..count {
		let transform = DAffine2::from_translation(direction * i as f64);
		for mut subpath in subpaths.clone() {
			subpath.apply_transform(transform);
			new_subpaths.push(subpath);
		}
	}

	vector_data.subpaths = new_subpaths;
	vector_data
}

#[derive(Debug, Clone, Copy)]
pub struct CircularRepeatNode<AngleOffset, Radius, Count> {
	angle_offset: AngleOffset,
	radius: Radius,
	count: Count,
}

#[node_macro::node_fn(CircularRepeatNode)]
fn circular_repeat_vector_data(mut vector_data: VectorData, angle_offset: f32, radius: f32, count: u32) -> VectorData {
	// repeat the vector data
	let VectorData { subpaths, transform, .. } = &vector_data;

	let mut new_subpaths: Vec<Subpath<_>> = Vec::with_capacity(subpaths.len() * count as usize);

	let bounding_box = vector_data.bounding_box().unwrap();
	let center = (bounding_box[0] + bounding_box[1]) / 2.;

	//let inverse = transform.inverse();
	//let radius_transform = DAffine2::from_translation(DVec2::new(0., radius as f64));
	let base_transform = DVec2::new(0., radius as f64) - center;

	for i in 0..count {
		let angle = (2. * std::f64::consts::PI / count as f64) * i as f64 + angle_offset.to_radians() as f64;
		let rotation = DAffine2::from_angle(angle);
		let transform = DAffine2::from_translation(center) * rotation * DAffine2::from_translation(base_transform);
		for mut subpath in subpaths.clone() {
			subpath.apply_transform(transform);
			new_subpaths.push(subpath);
		}
	}

	vector_data.subpaths = new_subpaths;
	vector_data
}

#[derive(Debug, Clone, Copy)]
pub struct BoundingBoxNode;

#[node_macro::node_fn(BoundingBoxNode)]
fn generate_bounding_box(vector_data: VectorData) -> VectorData {
	let bounding_box = vector_data.bounding_box().unwrap();
	VectorData::from_subpaths(vec![Subpath::new_rect(
		vector_data.transform.transform_point2(bounding_box[0]),
		vector_data.transform.transform_point2(bounding_box[1]),
	)])
}

#[derive(Debug, Clone, Copy)]
pub struct SetResampleCurveNode<Spacing> {
	spacing: Spacing,
}

#[node_macro::node_fn(SetResampleCurveNode)]
fn set_vector_data_resample_curve(mut vector_data: VectorData, spacing: f64) -> VectorData {
	vector_data.subpaths = vector_data
		.subpaths
		.iter()
		.map(|subpath| {
			let length = subpath.length(None);
			let rounded_count = (length / spacing).round();
			let difference = length - rounded_count * spacing;
			let adjusted_spacing = spacing + difference / rounded_count;

			Subpath::from_anchors(
				(0..=rounded_count as usize).map(|c| subpath.evaluate(SubpathTValue::GlobalEuclidean((c as f64 * adjusted_spacing / length).clamp(0.0, 0.99999)))),
				false,
			)
		})
		.collect();
	vector_data
}

#[derive(Debug, Clone, Copy)]
pub struct SetSplineFromPointsNode {}

#[node_macro::node_fn(SetSplineFromPointsNode)]
fn set_vector_data_spline_from_points(mut vector_data: VectorData) -> VectorData {
	let points: Vec<DVec2> = vector_data.subpaths.iter().flat_map(|subpath| subpath.anchors()).collect();

	vector_data.subpaths = if points.is_empty() { vec![] } else { vec![Subpath::new_cubic_spline(points)] };
	vector_data
}
