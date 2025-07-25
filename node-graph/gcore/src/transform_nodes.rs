use crate::raster_types::{CPU, GPU, Raster};
use crate::table::Table;
use crate::transform::{ApplyTransform, Footprint, Transform, TransformMut};
use crate::vector::Vector;
use crate::{CloneVarArgs, Context, Ctx, ExtractAll, Graphic, OwnedContextImpl};
use core::f64;
use glam::{DAffine2, DVec2};

/// An updated version of the transform node supporting selecting which instances/rows are transformed
#[node_macro::node(category(""))]
async fn transform_two<T: ApplyTransform2>(
	ctx: impl Ctx + CloneVarArgs + ExtractAll,
	#[implementations(
		Context -> DAffine2,
		Context -> DVec2,
		Context -> Table<Graphic>,
		Context -> Table<Vector>,
		Context -> Table<Raster<CPU>>,
		Context -> Table<Raster<GPU>>,
	)]
	value: impl Node<Context<'static>, Output = T>,
	translate: DVec2,
	rotate: f64,
	scale: DVec2,
	skew: DVec2,
	selection: impl Node<Context<'static>, Output = bool>,
) -> T {
	let matrix = DAffine2::from_scale_angle_translation(scale, rotate, translate) * DAffine2::from_cols_array(&[1., skew.y, skew.x, 1., 0., 0.]);

	let footprint = ctx.try_footprint().copied();

	let mut transform_target = {
		let mut new_ctx = OwnedContextImpl::from(ctx.clone());
		if let Some(mut footprint) = footprint {
			footprint.apply_transform(&matrix);
			new_ctx = new_ctx.with_footprint(footprint);
		}
		value.eval(new_ctx.into_context()).await
	};

	transform_target.apply_transformation(matrix, &ctx, selection).await;

	transform_target
}

/// A trait facilitating applying transforms with a particular selection field.
trait ApplyTransform2 {
	async fn apply_transformation<'n>(&mut self, matrix: DAffine2, ctx: &(impl Ctx + ExtractAll + CloneVarArgs), selection: &'n impl crate::Node<'n, Context<'n>, Output = impl Future<Output = bool>>);
}

/// Implementations of applying transforms for a table that implement the filtering based on the selection field.
impl<T> ApplyTransform2 for Table<T> {
	async fn apply_transformation<'n>(
		&mut self,
		matrix: DAffine2,
		ctx: &(impl Ctx + ExtractAll + CloneVarArgs),
		selection: &'n impl crate::Node<'n, Context<'n>, Output = impl Future<Output = bool>>,
	) {
		for (index, row) in self.iter_mut().enumerate() {
			let new_ctx = OwnedContextImpl::from(ctx.clone()).with_index(index);

			let should_eval = selection.eval(new_ctx.into_context()).await;
			if should_eval {
				info!("Applying to {index}");
				*row.transform = matrix * *row.transform;
			} else {
				info!("Skipping index {index}");
			}
		}
	}
}

/// An implementation for a non-table which ignores the selection
impl<T: TransformMut> ApplyTransform2 for T {
	async fn apply_transformation<'n>(&mut self, matrix: DAffine2, _: &(impl Ctx + ExtractAll + CloneVarArgs), _: &'n impl crate::Node<'n, Context<'n>, Output = impl Future<Output = bool>>) {
		*self.transform_mut() = matrix * self.transform();
	}
}

/// An implementation for a point which ignores the selection
impl ApplyTransform2 for DVec2 {
	async fn apply_transformation<'n>(&mut self, matrix: DAffine2, _: &(impl Ctx + ExtractAll + CloneVarArgs), _: &'n impl crate::Node<'n, Context<'n>, Output = impl Future<Output = bool>>) {
		*self = matrix.transform_point2(*self);
	}
}

#[node_macro::node(category(""))]
async fn transform<T: ApplyTransform + 'n + 'static>(
	ctx: impl Ctx + CloneVarArgs + ExtractAll,
	#[implementations(
		Context -> DAffine2,
		Context -> DVec2,
		Context -> Table<Vector>,
		Context -> Table<Graphic>,
		Context -> Table<Raster<CPU>>,
		Context -> Table<Raster<GPU>>,
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
	#[implementations(Table<Vector>, Table<Raster<CPU>>, Table<Graphic>)] mut data: Table<Data>,
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
	)]
	vector: Table<T>,
) -> DAffine2 {
	vector.iter_ref().next().map(|row| *row.transform).unwrap_or_default()
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
		Context -> Table<Vector>,
		Context -> Table<Graphic>,
		Context -> Table<Raster<CPU>>,
		Context -> Table<Raster<GPU>>,
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
		Context -> Table<Vector>,
		Context -> Table<Graphic>,
		Context -> Table<Raster<CPU>>,
		Context -> Table<Raster<GPU>>,
		Context -> String,
		Context -> f64,
	)]
	transform_target: impl Node<Context<'static>, Output = T>,
) -> T {
	let ctx = OwnedContextImpl::from(ctx).with_real_time(0.);

	transform_target.eval(ctx.into_context()).await
}
