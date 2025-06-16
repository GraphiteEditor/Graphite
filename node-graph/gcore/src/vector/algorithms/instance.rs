use crate::instances::{InstanceRef, Instances};
use crate::raster_types::{CPU, RasterDataTable};
use crate::vector::VectorDataTable;
use crate::{CloneVarArgs, Context, Ctx, ExtractAll, ExtractIndex, ExtractVarArgs, GraphicElement, GraphicGroupTable, OwnedContextImpl};
use glam::{DAffine2, DVec2};
use rand::{Rng, SeedableRng};
use std::f64::consts::TAU;

#[node_macro::node(category("Instancing"), path(graphene_core::vector))]
async fn repeat<I: 'n + Send + Clone>(
	_: impl Ctx,
	// TODO: Implement other GraphicElementRendered types.
	#[implementations(GraphicGroupTable, VectorDataTable, RasterDataTable<CPU>)] instance: Instances<I>,
	#[default(100., 100.)]
	// TODO: When using a custom Properties panel layout in document_node_definitions.rs and this default is set, the widget weirdly doesn't show up in the Properties panel. Investigation is needed.
	direction: PixelSize,
	angle: Angle,
	#[default(4)] instances: IntegerCount,
) -> Instances<I>
where
	Instances<I>: GraphicElementRendered,
{
	let angle = angle.to_radians();
	let count = instances.max(1);
	let total = (count - 1) as f64;

	let mut result_table = Instances::<I>::default();

	for index in 0..count {
		let angle = index as f64 * angle / total;
		let translation = index as f64 * direction / total;
		let transform = DAffine2::from_angle(angle) * DAffine2::from_translation(translation);

		for instance in instance.instance_ref_iter() {
			let mut instance = instance.to_instance_cloned();

			let local_translation = DAffine2::from_translation(instance.transform.translation);
			let local_matrix = DAffine2::from_mat2(instance.transform.matrix2);
			instance.transform = local_translation * transform * local_matrix;

			result_table.push(instance);
		}
	}

	result_table
}

#[node_macro::node(category("Instancing"), path(graphene_core::vector))]
async fn circular_repeat<I: 'n + Send + Clone>(
	_: impl Ctx,
	// TODO: Implement other GraphicElementRendered types.
	#[implementations(GraphicGroupTable, VectorDataTable, RasterDataTable<CPU>)] instance: Instances<I>,
	angle_offset: Angle,
	#[default(5)] radius: f64,
	#[default(5)] instances: IntegerCount,
) -> Instances<I>
where
	Instances<I>: GraphicElementRendered,
{
	let count = instances.max(1);

	let mut result_table = Instances::<I>::default();

	for index in 0..count {
		let angle = DAffine2::from_angle((TAU / count as f64) * index as f64 + angle_offset.to_radians());
		let translation = DAffine2::from_translation(radius * DVec2::Y);
		let transform = angle * translation;

		for instance in instance.instance_ref_iter() {
			let mut instance = instance.to_instance_cloned();

			let local_translation = DAffine2::from_translation(instance.transform.translation);
			let local_matrix = DAffine2::from_mat2(instance.transform.matrix2);
			instance.transform = local_translation * transform * local_matrix;

			result_table.push(instance);
		}
	}

	result_table
}

#[node_macro::node(name("Copy to Points"), category("Instancing"), path(graphene_core::vector))]
async fn copy_to_points<I: 'n + Send + Clone>(
	_: impl Ctx,
	points: VectorDataTable,
	#[expose]
	/// Artwork to be copied and placed at each point.
	#[implementations(GraphicGroupTable, VectorDataTable, RasterDataTable<CPU>)]
	instance: Instances<I>,
	/// Minimum range of randomized sizes given to each instance.
	#[default(1)]
	#[range((0., 2.))]
	#[unit("x")]
	random_scale_min: Multiplier,
	/// Maximum range of randomized sizes given to each instance.
	#[default(1)]
	#[range((0., 2.))]
	#[unit("x")]
	random_scale_max: Multiplier,
	/// Bias for the probability distribution of randomized sizes (0 is uniform, negatives favor more of small sizes, positives favor more of large sizes).
	#[range((-50., 50.))]
	random_scale_bias: f64,
	/// Seed to determine unique variations on all the randomized instance sizes.
	random_scale_seed: SeedValue,
	/// Range of randomized angles given to each instance, in degrees ranging from furthest clockwise to counterclockwise.
	#[range((0., 360.))]
	random_rotation: Angle,
	/// Seed to determine unique variations on all the randomized instance angles.
	random_rotation_seed: SeedValue,
) -> Instances<I>
where
	Instances<I>: GraphicElementRendered,
{
	let mut result_table = Instances::<I>::default();

	let random_scale_difference = random_scale_max - random_scale_min;

	for point_instance in points.instance_iter() {
		let mut scale_rng = rand::rngs::StdRng::seed_from_u64(random_scale_seed.into());
		let mut rotation_rng = rand::rngs::StdRng::seed_from_u64(random_rotation_seed.into());

		let do_scale = random_scale_difference.abs() > 1e-6;
		let do_rotation = random_rotation.abs() > 1e-6;

		let points_transform = point_instance.transform;
		for &point in point_instance.instance.point_domain.positions() {
			let translation = points_transform.transform_point2(point);

			let rotation = if do_rotation {
				let degrees = (rotation_rng.random::<f64>() - 0.5) * random_rotation;
				degrees / 360. * TAU
			} else {
				0.
			};

			let scale = if do_scale {
				if random_scale_bias.abs() < 1e-6 {
					// Linear
					random_scale_min + scale_rng.random::<f64>() * random_scale_difference
				} else {
					// Weighted (see <https://www.desmos.com/calculator/gmavd3m9bd>)
					let horizontal_scale_factor = 1. - 2_f64.powf(random_scale_bias);
					let scale_factor = (1. - scale_rng.random::<f64>() * horizontal_scale_factor).log2() / random_scale_bias;
					random_scale_min + scale_factor * random_scale_difference
				}
			} else {
				random_scale_min
			};

			let transform = DAffine2::from_scale_angle_translation(DVec2::splat(scale), rotation, translation);

			for mut instance in instance.instance_ref_iter().map(|instance| instance.to_instance_cloned()) {
				let local_matrix = DAffine2::from_mat2(instance.transform.matrix2);
				instance.transform = transform * local_matrix;

				result_table.push(instance);
			}
		}
	}

	result_table
}

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
	async fn repeat() {
		let direction = DVec2::X * 1.5;
		let instances = 3;
		let repeated = super::repeat(Footprint::default(), vector_node(Subpath::new_rect(DVec2::ZERO, DVec2::ONE)), direction, 0., instances).await;
		let vector_data = super::flatten_path(Footprint::default(), repeated).await;
		let vector_data = vector_data.instance_ref_iter().next().unwrap().instance;
		assert_eq!(vector_data.region_bezier_paths().count(), 3);
		for (index, (_, subpath)) in vector_data.region_bezier_paths().enumerate() {
			assert!((subpath.manipulator_groups()[0].anchor - direction * index as f64 / (instances - 1) as f64).length() < 1e-5);
		}
	}

	#[tokio::test]
	async fn circular_repeat() {
		let repeated = super::circular_repeat(Footprint::default(), vector_node(Subpath::new_rect(DVec2::NEG_ONE, DVec2::ONE)), 45., 4., 8).await;
		let vector_data = super::flatten_path(Footprint::default(), repeated).await;
		let vector_data = vector_data.instance_ref_iter().next().unwrap().instance;
		assert_eq!(vector_data.region_bezier_paths().count(), 8);

		for (index, (_, subpath)) in vector_data.region_bezier_paths().enumerate() {
			let expected_angle = (index as f64 + 1.) * 45.;

			let center = (subpath.manipulator_groups()[0].anchor + subpath.manipulator_groups()[2].anchor) / 2.;
			let actual_angle = DVec2::Y.angle_to(center).to_degrees();

			assert!((actual_angle - expected_angle).abs() % 360. < 1e-5, "Expected {expected_angle} found {actual_angle}");
		}
	}

	#[tokio::test]
	async fn copy_to_points() {
		let points = Subpath::new_rect(DVec2::NEG_ONE * 10., DVec2::ONE * 10.);
		let instance = Subpath::new_rect(DVec2::NEG_ONE, DVec2::ONE);

		let expected_points = VectorData::from_subpath(points.clone()).point_domain.positions().to_vec();

		let copy_to_points = super::copy_to_points(Footprint::default(), vector_node(points), vector_node(instance), 1., 1., 0., 0, 0., 0).await;
		let flatten_path = super::flatten_path(Footprint::default(), copy_to_points).await;
		let flattened_copy_to_points = flatten_path.instance_ref_iter().next().unwrap().instance;

		assert_eq!(flattened_copy_to_points.region_bezier_paths().count(), expected_points.len());

		for (index, (_, subpath)) in flattened_copy_to_points.region_bezier_paths().enumerate() {
			let offset = expected_points[index];
			assert_eq!(
				&subpath.anchors(),
				&[offset + DVec2::NEG_ONE, offset + DVec2::new(1., -1.), offset + DVec2::ONE, offset + DVec2::new(-1., 1.),]
			);
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
		let repeated = super::instance_on_points(owned, points, &rect, false).await;
		assert_eq!(repeated.len(), positions.len());
		for (position, instanced) in positions.into_iter().zip(repeated.instance_ref_iter()) {
			let bounds = instanced.instance.bounding_box_with_transform(*instanced.transform).unwrap();
			assert!(position.abs_diff_eq((bounds[0] + bounds[1]) / 2., 1e-10));
			assert_eq!((bounds[1] - bounds[0]).x, position.y);
		}
	}
}
