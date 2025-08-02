use crate::instances::Instances;
use crate::raster_types::{CPU, GPU, RasterDataTable};
use crate::transform::{ApplyTransform, Footprint, Transform};
use crate::vector::VectorDataTable;
use crate::{CloneVarArgs, Context, Ctx, ExtractAll, GraphicGroupTable, OwnedContextImpl};
use core::f64;
use glam::{DAffine2, DVec2};

#[node_macro::node(category(""))]
async fn transform<T: ApplyTransform + 'n + 'static>(
	ctx: impl Ctx + CloneVarArgs + ExtractAll,
	#[implementations(
		Context -> DAffine2,
		Context -> DVec2,
		Context -> VectorDataTable,
		Context -> GraphicGroupTable,
		Context -> RasterDataTable<CPU>,
		Context -> RasterDataTable<GPU>,
	)]
	value: impl Node<Context<'static>, Output = T>,
	translate: DVec2,
	rotate: f64,
	scale: DVec2,
	skew: DVec2,
) -> T {
	let matrix = DAffine2::from_scale_angle_translation(scale, rotate, translate) * DAffine2::from_cols_array(&[1., skew.y, skew.x, 1., 0., 0.]);

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
	_: impl Ctx,
	#[implementations(VectorDataTable, RasterDataTable<CPU>, GraphicGroupTable)] mut data: Instances<Data>,
	#[implementations(DAffine2)] transform: TransformInput,
) -> Instances<Data> {
	for data_transform in data.instance_mut_iter() {
		*data_transform.transform = transform.transform();
	}
	data
}

#[node_macro::node(category("Math: Transform"), path(graphene_core::vector))]
async fn extract_transform<T>(
	_: impl Ctx,
	#[implementations(
		GraphicGroupTable,
		VectorDataTable,
		RasterDataTable<CPU>,
		RasterDataTable<GPU>,
	)]
	vector_data: Instances<T>,
) -> DAffine2 {
	vector_data.instance_ref_iter().next().map(|vector_data| *vector_data.transform).unwrap_or_default()
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

#[node_macro::node(category("Debug"))]
async fn boundless_footprint<T: 'n + 'static>(
	ctx: impl Ctx + CloneVarArgs + ExtractAll,
	#[implementations(
		Context -> VectorDataTable,
		Context -> GraphicGroupTable,
		Context -> RasterDataTable<CPU>,
		Context -> RasterDataTable<GPU>,
		Context -> String,
		Context -> f64,
	)]
	transform_target: impl Node<Context<'static>, Output = T>,
) -> T {
	let ctx = OwnedContextImpl::from(ctx).with_footprint(Footprint::BOUNDLESS);

	transform_target.eval(ctx.into_context()).await
}

#[node_macro::node(category("Debug"))]
async fn freeze_real_time<T: 'n + 'static>(
	ctx: impl Ctx + CloneVarArgs + ExtractAll,
	#[implementations(
		Context -> VectorDataTable,
		Context -> GraphicGroupTable,
		Context -> RasterDataTable<CPU>,
		Context -> RasterDataTable<GPU>,
		Context -> String,
		Context -> f64,
	)]
	transform_target: impl Node<Context<'static>, Output = T>,
) -> T {
	let ctx = OwnedContextImpl::from(ctx).with_real_time(0.);

	transform_target.eval(ctx.into_context()).await
}
