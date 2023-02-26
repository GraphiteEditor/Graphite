use super::style::{Fill, FillType, Gradient, GradientType, Stroke};
use super::VectorData;
use crate::{Color, Node};
use glam::{DAffine2, DVec2};

#[derive(Debug, Clone, Copy)]
pub struct TransformNode<Translation, Rotation, Scale, Shear> {
	translate: Translation,
	rotate: Rotation,
	scale: Scale,
	shear: Shear,
}

#[node_macro::node_fn(TransformNode)]
fn transform_vector_data(mut vector_data: VectorData, translate: DVec2, rotate: f64, scale: DVec2, shear: DVec2) -> VectorData {
	let (sin, cos) = rotate.sin_cos();

	vector_data.transform = vector_data.transform * DAffine2::from_cols_array(&[scale.x + cos, shear.y + sin, shear.x - sin, scale.y + cos, translate.x, translate.y]);
	vector_data
}

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
	solid_color: Color,
	gradient_type: GradientType,
	start: DVec2,
	end: DVec2,
	transform: DAffine2,
	positions: Vec<(f64, Option<Color>)>,
) -> VectorData {
	vector_data.style.set_fill(match fill_type {
		FillType::None => Fill::None,
		FillType::Solid => Fill::Solid(solid_color),
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
	color: crate::Color,
	weight: f64,
	dash_lengths: Vec<f32>,
	dash_offset: f64,
	line_cap: super::style::LineCap,
	line_join: super::style::LineJoin,
	miter_limit: f64,
) -> VectorData {
	vector_data.style.set_stroke(Stroke {
		color: Some(color),
		weight,
		dash_lengths,
		dash_offset,
		line_cap,
		line_join,
		line_join_miter_limit: miter_limit,
	});
	vector_data
}
