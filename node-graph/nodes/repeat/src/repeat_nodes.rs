use crate::gcore::Context;
use core::f64::consts::TAU;
use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::registry::types::{Angle, IntegerCount, PixelSize};
use core_types::table::{Table, TableRowRef};
use core_types::{CloneVarArgs, Color, Ctx, ExtractAll, InjectVarArgs, OwnedContextImpl};
use glam::{DAffine2, DVec2};
use graphic_types::{Graphic, Vector};
use raster_types::{CPU, Raster};
use vector_types::{GradientStops, ReferencePoint};

#[node_macro::node(category("Repeat"), path(core_types::vector))]
async fn repeat<T: Into<Graphic> + Default + Send + Clone + 'static>(
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
	// Someday this node can have the option to generate infinitely instead of a fixed count (basically `std::iter::repeat`).

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

#[node_macro::node(category("Repeat"), path(core_types::vector))]
async fn linear_repeat<T: Into<Graphic> + Default + Send + Clone + 'static>(
	ctx: impl ExtractAll + CloneVarArgs + Ctx,
	#[implementations(
		Context -> Table<Graphic>,
		Context -> Table<Vector>,
		Context -> Table<Raster<CPU>>,
		Context -> Table<Color>,
		Context -> Table<GradientStops>,
	)]
	instance: impl Node<'n, Context<'static>, Output = Table<T>>,
	#[default(100., 100.)]
	// TODO: When using a custom Properties panel layout in document_node_definitions.rs and this default is set, the widget weirdly doesn't show up in the Properties panel. Investigation is needed.
	direction: PixelSize,
	angle: Angle,
	#[default(5)] count: IntegerCount,
) -> Table<T> {
	let angle = angle.to_radians();
	let count = count.max(1);
	let total = (count - 1) as f64;

	let mut result_table = Table::new();

	for index in 0..count {
		let angle = index as f64 * angle / total;
		let translation = index as f64 * direction / total;
		let transform = DAffine2::from_angle(angle) * DAffine2::from_translation(translation);

		let new_ctx = OwnedContextImpl::from(ctx.clone()).with_index(index as usize);
		let generated_instance = instance.eval(new_ctx.into_context()).await;

		for row in generated_instance.iter() {
			let mut row = row.into_cloned();

			let local_translation = DAffine2::from_translation(row.transform.translation);
			let local_matrix = DAffine2::from_mat2(row.transform.matrix2);
			row.transform = local_translation * transform * local_matrix;

			result_table.push(row);
		}
	}

	result_table
}

#[node_macro::node(category("Repeat"), path(core_types::vector))]
async fn radial_repeat<T: Into<Graphic> + Default + Send + Clone + 'static>(
	ctx: impl ExtractAll + CloneVarArgs + Ctx,
	#[implementations(
		Context -> Table<Graphic>,
		Context -> Table<Vector>,
		Context -> Table<Raster<CPU>>,
		Context -> Table<Color>,
		Context -> Table<GradientStops>,
	)]
	instance: impl Node<'n, Context<'static>, Output = Table<T>>,
	start_angle: Angle,
	#[unit(" px")]
	#[default(5)]
	radius: f64,
	#[default(5)] count: IntegerCount,
) -> Table<T> {
	let count = count.max(1);

	let mut result_table = Table::new();

	for index in 0..count {
		let angle = DAffine2::from_angle((TAU / count as f64) * index as f64 + start_angle.to_radians());
		let translation = DAffine2::from_translation(radius * DVec2::Y);
		let transform = angle * translation;

		let new_ctx = OwnedContextImpl::from(ctx.clone()).with_index(index as usize);
		let generated_instance = instance.eval(new_ctx.into_context()).await;

		for row in generated_instance.iter() {
			let mut row = row.into_cloned();

			let local_translation = DAffine2::from_translation(row.transform.translation);
			let local_matrix = DAffine2::from_mat2(row.transform.matrix2);
			row.transform = local_translation * transform * local_matrix;

			result_table.push(row);
		}
	}

	result_table
}

#[node_macro::node(category("Repeat"), path(core_types::vector))]
async fn mirror_repeat<T: 'n + Send + Clone>(
	_: impl Ctx,
	#[implementations(
		Table<Graphic>,
		Table<Vector>,
		Table<Raster<CPU>>,
		Table<Color>,
		Table<GradientStops>,
	)]
	instance: Table<T>,
	#[default(ReferencePoint::Center)] relative_to_bounds: ReferencePoint,
	#[unit(" px")] offset: f64,
	#[range((-90., 90.))] angle: Angle,
	#[default(true)] keep_original: bool,
) -> Table<T>
where
	Table<T>: BoundingBox,
{
	// Normalize the direction vector
	let normal = DVec2::from_angle(angle.to_radians());

	// The mirror reference may be based on the bounding box if an explicit reference point is chosen
	let RenderBoundingBox::Rectangle(bounding_box) = instance.bounding_box(DAffine2::IDENTITY, false) else {
		return instance;
	};

	let reference_point_location = relative_to_bounds.point_in_bounding_box((bounding_box[0], bounding_box[1]).into());
	let mirror_reference_point = reference_point_location.map(|point| point + normal * offset);

	// Create the reflection matrix
	let reflection = DAffine2::from_mat2_translation(
		glam::DMat2::from_cols(
			DVec2::new(1. - 2. * normal.x * normal.x, -2. * normal.y * normal.x),
			DVec2::new(-2. * normal.x * normal.y, 1. - 2. * normal.y * normal.y),
		),
		DVec2::ZERO,
	);

	// Apply reflection around the reference point
	let reflected_transform = if let Some(mirror_reference_point) = mirror_reference_point {
		DAffine2::from_translation(mirror_reference_point) * reflection * DAffine2::from_translation(-mirror_reference_point)
	} else {
		reflection * DAffine2::from_translation(DVec2::from_angle(angle.to_radians()) * DVec2::splat(-offset))
	};

	let mut result_table = Table::new();

	// Add original instance depending on the keep_original flag
	if keep_original {
		for instance in instance.clone().into_iter() {
			result_table.push(instance);
		}
	}

	// Create and add mirrored instance
	for mut row in instance.into_iter() {
		row.transform = reflected_transform * row.transform;
		result_table.push(row);
	}

	result_table
}

#[node_macro::node(category("Repeat"), path(core_types::vector))]
async fn pointwise_repeat<T: Into<Graphic> + Default + Send + Clone + 'static>(
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

			let new_ctx = OwnedContextImpl::from(ctx.clone()).with_index(index).with_position(transformed_point);
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
