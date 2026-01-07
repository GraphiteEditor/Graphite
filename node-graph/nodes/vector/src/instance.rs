use core_types::Color;
use core_types::table::{Table, TableRowRef};
use core_types::{CloneVarArgs, Context, Ctx, ExtractAll, ExtractIndex, ExtractVarArgs, InjectVarArgs, OwnedContextImpl};
use glam::DVec2;
use graphic_types::Graphic;
use graphic_types::Vector;
use graphic_types::raster_types::{CPU, Raster};
use vector_types::GradientStops;

use log::*;

#[repr(transparent)]
#[derive(dyn_any::DynAny)]
struct HashableDVec2(DVec2);

impl std::hash::Hash for HashableDVec2 {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.0.x.to_bits().hash(state);
		self.0.y.to_bits().hash(state);
	}
}

#[node_macro::node(name("Instance on Points"), category("Instancing"), path(core_types::vector))]
async fn instance_on_points<T: Into<Graphic> + Default + Send + Clone + 'static>(
	ctx: impl ExtractAll + CloneVarArgs + Sync + Ctx + InjectVarArgs,
	points: Table<Vector>,
	#[implementations(
		Context -> Table<Graphic>,
		Context -> Table<Vector>,
		Context -> Table<Raster<CPU>>,
		Context -> Table<Color>,
		Context -> Table<GradientStops>,
	)]
	instance: impl Node<'n, Context<'static>, Output = Table<T>>,
	reverse: bool,
) -> Table<T> {
	let mut result_table = Table::new();

	for TableRowRef { element: points, transform, .. } in points.iter() {
		let mut iteration = async |index, point| {
			let transformed_point = transform.transform_point2(point);

			let new_ctx = OwnedContextImpl::from(ctx.clone()).with_index(index).with_vararg(Box::new(HashableDVec2(transformed_point)));
			let generated_instance = instance.eval(new_ctx.into_context()).await;

			for mut generated_row in generated_instance.into_iter() {
				generated_row.transform.translation = transformed_point;
				result_table.push(generated_row);
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

#[node_macro::node(category("Instancing"), path(core_types::vector))]
async fn instance_repeat<T: Into<Graphic> + Default + Send + Clone + 'static>(
	ctx: impl ExtractAll + CloneVarArgs + Ctx,
	#[implementations(
		Context -> Table<Graphic>,
		Context -> Table<Vector>,
		Context -> Table<Raster<CPU>>,
		Context -> Table<Color>,
		Context -> Table<GradientStops>,
	)]
	instance: impl Node<'n, Context<'static>, Output = Table<T>>,
	#[default(1)] count: u64,
	reverse: bool,
) -> Table<T> {
	let count = count.max(1) as usize;

	let mut result_table = Table::new();

	for index in 0..count {
		let index = if reverse { count - index - 1 } else { index };

		let new_ctx = OwnedContextImpl::from(ctx.clone()).with_index(index);
		let generated_instance = instance.eval(new_ctx.into_context()).await;

		for generated_row in generated_instance.into_iter() {
			result_table.push(generated_row);
		}
	}

	result_table
}

#[node_macro::node(category("Instancing"), path(core_types::vector))]
async fn instance_position(ctx: impl Ctx + ExtractVarArgs) -> DVec2 {
	match ctx.vararg(0).map(|dynamic| dynamic.downcast_ref::<HashableDVec2>()) {
		Ok(Some(position)) => return position.0,
		Ok(_) => warn!("Extracted value of incorrect type"),
		Err(e) => warn!("Cannot extract position vararg: {e:?}"),
	}
	Default::default()
}

// TODO: Return u32, u64, or usize instead of f64 after #1621 is resolved and has allowed us to implement automatic type conversion in the node graph for nodes with generic type inputs.
// TODO: (Currently automatic type conversion only works for concrete types, via the Graphene preprocessor and not the full Graphene type system.)
#[node_macro::node(category("Instancing"), path(core_types::vector))]
async fn instance_index(ctx: impl Ctx + ExtractIndex, _primary: (), loop_level: u32) -> f64 {
	let Some(index_iter) = ctx.try_index() else { return 0. };
	let mut last = 0;
	for (i, index) in index_iter.enumerate() {
		if i == loop_level as usize {
			return index as f64;
		}
		last = index;
	}
	last as f64
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::generator_nodes::RectangleNode;
	use core_types::Ctx;
	use core_types::Node;
	use glam::DVec2;
	use graphene_core::extract_xy::{ExtractXyNode, XY};
	use graphic_types::Vector;
	use std::future::Future;
	use std::pin::Pin;
	use vector_types::subpath::Subpath;

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
		let rect = RectangleNode::new(
			FutureWrapperNode(()),
			ExtractXyNode::new(InstancePositionNode {}, FutureWrapperNode(XY::Y)),
			FutureWrapperNode(2_f64),
			FutureWrapperNode(false),
			FutureWrapperNode(0_f64),
			FutureWrapperNode(false),
		);

		let positions = [DVec2::new(40., 20.), DVec2::ONE, DVec2::new(-42., 9.), DVec2::new(10., 345.)];
		let points = Table::new_from_element(Vector::from_subpath(Subpath::from_anchors(positions, false)));
		let generated = super::instance_on_points(owned, points, &rect, false).await;
		assert_eq!(generated.len(), positions.len());
		for (position, generated_row) in positions.into_iter().zip(generated.iter()) {
			let bounds = generated_row.element.bounding_box_with_transform(*generated_row.transform).unwrap();
			assert!(position.abs_diff_eq((bounds[0] + bounds[1]) / 2., 1e-10));
			assert_eq!((bounds[1] - bounds[0]).x, position.y);
		}
	}
}
