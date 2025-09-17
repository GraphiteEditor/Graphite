use crate::gradient::GradientStops;
use crate::raster_types::{CPU, GPU, Raster};
use crate::table::Table;
use crate::transform::{ApplyTransform, Transform};
use crate::vector::Vector;
use crate::{CloneVarArgs, Context, Ctx, ExtractAll, Graphic, InjectFootprint, ModifyFootprint, OwnedContextImpl};
use core::f64;
use glam::{DAffine2, DVec2};
use graphene_core_shaders::color::Color;

#[node_macro::node(category(""))]
async fn transform<T: ApplyTransform + 'n + 'static>(
	ctx: impl Ctx + CloneVarArgs + ExtractAll + ModifyFootprint,
	#[implementations(
		Context -> DAffine2,
		Context -> DVec2,
		Context -> Table<Vector>,
		Context -> Table<Graphic>,
		Context -> Table<Raster<CPU>>,
		Context -> Table<Raster<GPU>>,
		Context -> Table<Color>,
		Context -> Table<GradientStops>,
	)]
	value: impl Node<Context<'static>, Output = T>,
	translation: DVec2,
	rotation: f64,
	scale: DVec2,
	skew: DVec2,
) -> T {
	let trs = DAffine2::from_scale_angle_translation(scale, rotation.to_radians(), translation);
	let skew = DAffine2::from_cols_array(&[1., skew.y.to_radians().tan(), skew.x.to_radians().tan(), 1., 0., 0.]);
	let matrix = trs * skew;

	let footprint = ctx.try_footprint().copied();

	let mut ctx = OwnedContextImpl::from(ctx);
	if let Some(mut footprint) = footprint {
		footprint.apply_transform(&matrix);
		ctx = ctx.with_footprint(footprint);
	}

	let mut transform_target = value.eval(ctx.into_context()).await;

	transform_target.left_apply_transform(&matrix);

	transform_target
}

#[node_macro::node(category(""))]
fn replace_transform<Data, TransformInput: Transform>(
	_: impl Ctx + InjectFootprint,
	#[implementations(Table<Vector>, Table<Raster<CPU>>, Table<Graphic>, Table<Color>, Table<GradientStops>)] mut data: Table<Data>,
	#[implementations(DAffine2)] transform: TransformInput,
) -> Table<Data> {
	for data_transform in data.iter_mut() {
		*data_transform.transform = transform.transform();
	}
	data
}

#[node_macro::node(category("Math: Transform"), path(graphene_core::vector))]
async fn extract_transform<T>(
	_: impl Ctx,
	#[implementations(
		Table<Graphic>,
		Table<Vector>,
		Table<Raster<CPU>>,
		Table<Raster<GPU>>,
		Table<Color>,
		Table<GradientStops>,
	)]
	vector: Table<T>,
) -> DAffine2 {
	vector.iter().next().map(|row| *row.transform).unwrap_or_default()
}

#[node_macro::node(category("Math: Transform"))]
fn invert_transform(_: impl Ctx, transform: DAffine2) -> DAffine2 {
	transform.inverse()
}

#[node_macro::node(category("Math: Transform"))]
fn decompose_translation(_: impl Ctx, transform: DAffine2) -> DVec2 {
	transform.translation
}

#[node_macro::node(category("Math: Transform"))]
fn decompose_rotation(_: impl Ctx, transform: DAffine2) -> f64 {
	transform.decompose_rotation()
}

#[node_macro::node(category("Math: Transform"))]
fn decompose_scale(_: impl Ctx, transform: DAffine2) -> DVec2 {
	transform.decompose_scale()
}
