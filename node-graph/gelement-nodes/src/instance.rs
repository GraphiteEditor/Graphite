use glam::DVec2;
use graphene_core::GraphicElement;
use graphene_core::GraphicGroupTable;
use graphene_core::context::{CloneVarArgs, Context, Ctx, ExtractAll, ExtractIndex, ExtractVarArgs, OwnedContextImpl};
use graphene_core::instances::{InstanceRef, Instances};
use graphene_core::raster_types::{CPU, RasterDataTable};
use graphene_core::vector::VectorDataTable;
use log::warn;

#[node_macro::node(name("Instance on Points"), category("Instancing"), path(graphene_core::vector))]
async fn instance_on_points<T: Into<GraphicElement> + Default + Send + Clone + 'static>(
	ctx: impl ExtractAll + CloneVarArgs + Sync + Ctx,
	points: VectorDataTable,
	#[implementations(
		Context -> GraphicGroupTable,
		Context -> VectorDataTable,
		Context -> RasterDataTable<CPU>
	)]
	instance: impl Node<'n, Context<'static>, Output = Instances<T>>,
	reverse: bool,
) -> Instances<T> {
	let mut result_table = Instances::<T>::default();

	for InstanceRef { instance: points, transform, .. } in points.instance_ref_iter() {
		let mut iteration = async |index, point| {
			let transformed_point = transform.transform_point2(point);

			let new_ctx = OwnedContextImpl::from(ctx.clone()).with_index(index).with_vararg(Box::new(transformed_point));
			let generated_instance = instance.eval(new_ctx.into_context()).await;

			for mut instanced in generated_instance.instance_iter() {
				instanced.transform.translation = transformed_point;
				result_table.push(instanced);
			}
		};

		let range = points.point_domain.positions().iter().enumerate();
		if reverse {
			for (index, &point) in range.rev() {
				iteration(index, point).await;
			}
		} else {
			for (index, &point) in range {
				iteration(index, point).await;
			}
		}
	}

	result_table
}

#[node_macro::node(category("Instancing"), path(graphene_core::vector))]
async fn instance_repeat<T: Into<GraphicElement> + Default + Send + Clone + 'static>(
	ctx: impl ExtractAll + CloneVarArgs + Ctx,
	#[implementations(
		Context -> GraphicGroupTable,
		Context -> VectorDataTable,
		Context -> RasterDataTable<CPU>
	)]
	instance: impl Node<'n, Context<'static>, Output = Instances<T>>,
	#[default(1)] count: u64,
	reverse: bool,
) -> Instances<T> {
	let count = count.max(1) as usize;

	let mut result_table = Instances::<T>::default();

	for index in 0..count {
		let index = if reverse { count - index - 1 } else { index };

		let new_ctx = OwnedContextImpl::from(ctx.clone()).with_index(index);
		let generated_instance = instance.eval(new_ctx.into_context()).await;

		for instanced in generated_instance.instance_iter() {
			result_table.push(instanced);
		}
	}

	result_table
}

#[node_macro::node(category("Instancing"), path(graphene_core::vector))]
async fn instance_position(ctx: impl Ctx + ExtractVarArgs) -> DVec2 {
	match ctx.vararg(0).map(|dynamic| dynamic.downcast_ref::<DVec2>()) {
		Ok(Some(position)) => return *position,
		Ok(_) => warn!("Extracted value of incorrect type"),
		Err(e) => warn!("Cannot extract position vararg: {e:?}"),
	}
	Default::default()
}

#[node_macro::node(category("Instancing"), path(graphene_core::vector))]
async fn instance_index(ctx: impl Ctx + ExtractIndex) -> f64 {
	match ctx.try_index() {
		Some(index) => return index as f64,
		None => warn!("Extracted value of incorrect type"),
	}
	0.
}

#[cfg(test)]
mod test {
	use super::*;
	use bezier_rs::Subpath;
	use glam::DVec2;
	use graphene_core::Node;
	use graphene_core::extract_xy::{ExtractXyNode, XY};
	use graphene_core::vector::VectorData;
	use std::pin::Pin;

	#[derive(Clone)]
	pub struct FutureWrapperNode<T: Clone>(T);

	impl<'i, I: Ctx, T: 'i + Clone + Send> Node<'i, I> for FutureWrapperNode<T> {
		type Output = Pin<Box<dyn Future<Output = T> + 'i + Send>>;
		fn eval(&'i self, _input: I) -> Self::Output {
			let value = self.0.clone();
			Box::pin(async move { value })
		}
	}

	#[tokio::test]
	async fn instance_on_points_test() {
		let owned = OwnedContextImpl::default().into_context();
		let rect = graphene_core::vector::generator_nodes::RectangleNode::new(
			FutureWrapperNode(()),
			ExtractXyNode::new(InstancePositionNode {}, FutureWrapperNode(XY::Y)),
			FutureWrapperNode(2_f64),
			FutureWrapperNode(false),
			FutureWrapperNode(0_f64),
			FutureWrapperNode(false),
		);

		let positions = [DVec2::new(40., 20.), DVec2::ONE, DVec2::new(-42., 9.), DVec2::new(10., 345.)];
		let points = VectorDataTable::new(VectorData::from_subpath(Subpath::from_anchors_linear(positions, false)));
		let repeated = super::instance_on_points(owned, points, &rect, false).await;
		assert_eq!(repeated.len(), positions.len());
		for (position, instanced) in positions.into_iter().zip(repeated.instance_ref_iter()) {
			let bounds = instanced.instance.bounding_box_with_transform(*instanced.transform).unwrap();
			assert!(position.abs_diff_eq((bounds[0] + bounds[1]) / 2., 1e-10));
			assert_eq!((bounds[1] - bounds[0]).x, position.y);
		}
	}
}
