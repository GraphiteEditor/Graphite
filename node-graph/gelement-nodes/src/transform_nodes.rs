use core::f64;
use glam::{DAffine2, DVec2};
use graphene_core::GraphicGroupTable;
use graphene_core::context::{CloneVarArgs, Context, Ctx, ExtractAll, OwnedContextImpl};
use graphene_core::instances::Instances;
use graphene_core::raster_types::{CPU, GPU, RasterDataTable};
use graphene_core::transform::{ApplyTransform, Footprint, Transform};
use graphene_core::vector::VectorDataTable;

#[node_macro::node(category(""))]
async fn transform<T: 'n + 'static>(
	ctx: impl Ctx + CloneVarArgs + ExtractAll,
	#[implementations(
		Context -> VectorDataTable,
		Context -> GraphicGroupTable,
		Context -> RasterDataTable<CPU>,
		Context -> RasterDataTable<GPU>,
	)]
	transform_target: impl Node<Context<'static>, Output = Instances<T>>,
	translate: DVec2,
	rotate: f64,
	scale: DVec2,
	shear: DVec2,
	_pivot: DVec2,
) -> Instances<T> {
	let matrix = DAffine2::from_scale_angle_translation(scale, rotate, translate) * DAffine2::from_cols_array(&[1., shear.y, shear.x, 1., 0., 0.]);

	let footprint = ctx.try_footprint().copied();

	let mut ctx = OwnedContextImpl::from(ctx);
	if let Some(mut footprint) = footprint {
		footprint.apply_transform(&matrix);
		ctx = ctx.with_footprint(footprint);
	}

	let mut transform_target = transform_target.eval(ctx.into_context()).await;

	for data_transform in transform_target.instance_mut_iter() {
		*data_transform.transform = matrix * *data_transform.transform;
	}

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
