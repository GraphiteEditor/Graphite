use crate::instances::Instances;
use crate::raster_types::{CPU, GPU, RasterDataTable};
use crate::transform::{Transform};
use crate::vector::VectorDataTable;
use crate::{ Context, Ctx, GraphicGroupTable, ModifyDownstreamTransform, };
use core::f64;
use glam::{DAffine2, DVec2};

#[node_macro::node(category(""))]
async fn transform<T: 'n + 'static>(
	ctx: impl Ctx + ModifyDownstreamTransform,
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
	skew: DVec2,
) -> Instances<T> {
	let matrix = DAffine2::from_scale_angle_translation(scale, rotate, translate) * DAffine2::from_cols_array(&[1., skew.y, skew.x, 1., 0., 0.]);

	let modified_ctx = ctx.apply_modification(&matrix);

	let mut transform_target = transform_target.eval(modified_ctx).await;

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
