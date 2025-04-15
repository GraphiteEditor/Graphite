use glam::{DAffine2, DVec2};

use crate::{CloneVarArgs, Context, Ctx, ExtractAll, ExtractIndex, ExtractVarArgs, OwnedContextImpl, instances::Instance, vector::VectorDataTable};

#[node_macro::node(name("Instance on Points"), category("Vector: Shape"), path(graphene_core::vector))]
async fn instance_on_points(
	ctx: impl ExtractAll + CloneVarArgs + Ctx,
	points: VectorDataTable,
	#[implementations(Context -> VectorDataTable)] instance_node: impl Node<'n, Context<'static>, Output = VectorDataTable>,
) -> VectorDataTable {
	let mut result = VectorDataTable::empty();

	for Instance { instance: points, transform, .. } in points.instances() {
		for (index, &point) in points.point_domain.positions().iter().enumerate() {
			let transformed_point = transform.transform_point2(point);
			println!("Transformed {transformed_point:?}");
			let new_ctx = OwnedContextImpl::from(ctx.clone()).with_index(index).with_vararg(Box::new(transformed_point));
			let instanced = instance_node.eval(new_ctx.into_context()).await;
			for instanced in instanced.instances() {
				let instanced = result.push_instance(instanced);
				*instanced.transform *= DAffine2::from_translation(transformed_point);
			}
		}
	}
	result
}

#[node_macro::node(category("Attributes"), path(graphene_core::vector))]
async fn instance_position(ctx: impl Ctx + ExtractVarArgs) -> DVec2 {
	match ctx.vararg(0).map(|dynamic| dynamic.downcast_ref::<DVec2>()) {
		Ok(Some(position)) => return *position,
		Ok(_) => warn!("Extracted value of incorrect type"),
		Err(e) => warn!("Cannot extract position vararg: {e:?}"),
	}
	Default::default()
}

#[node_macro::node(category("Attributes"), path(graphene_core::vector))]
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
	use crate::Node;
	use crate::ops::ExtractXyNode;
	use crate::vector::VectorData;
	use bezier_rs::Subpath;
	use glam::DVec2;
	use std::pin::Pin;

	#[derive(Clone)]
	pub struct FutureWrapperNode<T: Clone>(T);

	impl<'i, I: Ctx, T: 'i + Clone + Send> Node<'i, I> for FutureWrapperNode<T> {
		type Output = Pin<Box<dyn core::future::Future<Output = T> + 'i + Send>>;
		fn eval(&'i self, _input: I) -> Self::Output {
			let value = self.0.clone();
			Box::pin(async move { value })
		}
	}

	#[tokio::test]
	async fn instance_on_points_test() {
		let owned = OwnedContextImpl::default().into_context();
		let rect = crate::vector::generator_nodes::RectangleNode::new(
			FutureWrapperNode(()),
			ExtractXyNode::new(InstancePositionNode {}, FutureWrapperNode(crate::ops::XY::Y)),
			FutureWrapperNode(2_f64),
			FutureWrapperNode(false),
			FutureWrapperNode(0_f64),
			FutureWrapperNode(false),
		);

		let positions = [DVec2::new(40., 20.), DVec2::ONE, DVec2::new(-42., 9.), DVec2::new(10., 345.)];
		let points = VectorDataTable::new(VectorData::from_subpath(Subpath::from_anchors_linear(positions, false)));
		let repeated = super::instance_on_points(owned, points, &rect).await;
		assert_eq!(repeated.len(), positions.len());
		for (position, instanced) in positions.into_iter().zip(repeated.instances()) {
			let bounds = instanced.instance.bounding_box_with_transform(*instanced.transform).unwrap();
			assert!(position.abs_diff_eq((bounds[0] + bounds[1]) / 2., 1e-10));
			assert_eq!((bounds[1] - bounds[0]).x, position.y);
		}
	}
}
