use crate::gcore::Context;
use core::f64::consts::TAU;
use core_types::gpoll::Interrupt;
use core_types::list::List;
use core_types::registry::types::{Angle, PixelSize};
use core_types::{ATTR_TRANSFORM, Color, Ctx, DeriveCtx, InjectVarArgs};
use glam::{DAffine2, DVec2};
use graphic_types::{Graphic, Vector};
use raster_types::{CPU, Raster};
use vector_types::GradientStops;

#[node_macro::node(category("Repeat"))]
fn repeat<T: Into<Graphic> + Default + Send + Clone + 'static>(
	ctx: impl Ctx + DeriveCtx,
	#[implementations(
		Context -> List<Graphic>,
		Context -> List<Vector>,
		Context -> List<Raster<CPU>>,
		Context -> List<Color>,
		Context -> List<GradientStops>,
	)]
	content: impl Node<Context<'_>, Output = List<T>>,
	#[default(1)]
	#[hard(1..)]
	count: u32,
	reverse: bool,
) -> Result<List<T>, Interrupt> {
	// Someday this node can have the option to generate infinitely instead of a fixed count (basically `std::iter::repeat`).

	let count = count as u64;
	let spilled = ctx.index_head();

	let mut result_list = List::new();

	for index in 0..count {
		let index = if reverse { count - index - 1 } else { index };

		let generated_content = content.eval(&ctx.promoted(&spilled, index))?;

		for generated_row in generated_content.into_iter() {
			result_list.push(generated_row);
		}
	}

	Ok(result_list)
}

#[node_macro::node(category("Repeat"))]
pub fn repeat_array<T: Into<Graphic> + Default + Send + Clone + 'static>(
	ctx: impl Ctx + DeriveCtx,
	#[implementations(
		Context -> List<Graphic>,
		Context -> List<Vector>,
		Context -> List<Raster<CPU>>,
		Context -> List<Color>,
		Context -> List<GradientStops>,
	)]
	content: impl Node<Context<'_>, Output = List<T>>,
	#[default(100., 100.)]
	// TODO: When using a custom Properties panel layout in document_node_definitions.rs and this default is set, the widget weirdly doesn't show up in the Properties panel. Investigation is needed.
	direction: PixelSize,
	angle: Angle,
	#[default(5)]
	#[hard(1..)]
	count: u32,
) -> Result<List<T>, Interrupt> {
	let angle = angle.to_radians();
	// A single copy has no steps between copies, so the denominator is kept at 1 to avoid `0. / 0.` producing a NaN transform
	let total = (count - 1).max(1) as f64;
	let spilled = ctx.index_head();

	let mut result_list = List::new();

	for index in 0..count {
		let angle = index as f64 * angle / total;
		let translation = index as f64 * direction / total;
		let transform = DAffine2::from_angle(angle) * DAffine2::from_translation(translation);

		let generated_content = content.eval(&ctx.promoted(&spilled, index as u64))?;

		for row_index in 0..generated_content.len() {
			let Some(mut row) = generated_content.clone_item(row_index) else { continue };

			let local_transform: DAffine2 = row.attribute_cloned_or_default(ATTR_TRANSFORM);
			let local_translation = DAffine2::from_translation(local_transform.translation);
			let local_matrix = DAffine2::from_mat2(local_transform.matrix2);
			*row.attribute_mut_or_insert_default(ATTR_TRANSFORM) = local_translation * transform * local_matrix;

			result_list.push(row);
		}
	}

	Ok(result_list)
}

#[node_macro::node(category("Repeat"))]
fn repeat_radial<T: Into<Graphic> + Default + Send + Clone + 'static>(
	ctx: impl Ctx + DeriveCtx,
	#[implementations(
		Context -> List<Graphic>,
		Context -> List<Vector>,
		Context -> List<Raster<CPU>>,
		Context -> List<Color>,
		Context -> List<GradientStops>,
	)]
	content: impl Node<Context<'_>, Output = List<T>>,
	start_angle: Angle,
	#[unit(" px")]
	#[default(5)]
	radius: f64,
	#[default(5)]
	#[hard(1..)]
	count: u32,
) -> Result<List<T>, Interrupt> {
	let spilled = ctx.index_head();
	let mut result_list = List::new();

	for index in 0..count {
		let angle = DAffine2::from_angle((TAU / count as f64) * index as f64 + start_angle.to_radians());
		let translation = DAffine2::from_translation(radius * DVec2::Y);
		let transform = angle * translation;

		let generated_content = content.eval(&ctx.promoted(&spilled, index as u64))?;

		for row_index in 0..generated_content.len() {
			let Some(mut row) = generated_content.clone_item(row_index) else { continue };

			let local_transform: DAffine2 = row.attribute_cloned_or_default(ATTR_TRANSFORM);
			let local_translation = DAffine2::from_translation(local_transform.translation);
			let local_matrix = DAffine2::from_mat2(local_transform.matrix2);
			*row.attribute_mut_or_insert_default(ATTR_TRANSFORM) = local_translation * transform * local_matrix;

			result_list.push(row);
		}
	}

	Ok(result_list)
}

#[node_macro::node(category("Repeat"), name("Repeat on Points"))]
fn repeat_on_points<T: Into<Graphic> + Default + Send + Clone + 'static>(
	ctx: impl Ctx + DeriveCtx + InjectVarArgs,
	points: List<Vector>,
	#[implementations(
		Context -> List<Graphic>,
		Context -> List<Vector>,
		Context -> List<Raster<CPU>>,
		Context -> List<Color>,
		Context -> List<GradientStops>,
	)]
	content: impl Node<Context<'_>, Output = List<T>>,
	reverse: bool,
) -> Result<List<T>, Interrupt> {
	let spilled = ctx.index_head();
	let mut result_list = List::new();

	for points_index in 0..points.len() {
		let Some(points_element) = points.element(points_index) else { continue };
		let transform: DAffine2 = points.attribute_cloned_or_default(ATTR_TRANSFORM, points_index);

		let positions = points_element.point_domain.positions();
		let range: Box<dyn Iterator<Item = (usize, &DVec2)>> = match reverse {
			true => Box::new(positions.iter().enumerate().rev()),
			false => Box::new(positions.iter().enumerate()),
		};

		for (index, &point) in range {
			let transformed_point = transform.transform_point2(point);

			let scoped = ctx.push_position(transformed_point);
			let generated_content = content.eval(&scoped.ctx().promoted(&spilled, index as u64))?;

			for mut generated_row in generated_content.into_iter() {
				generated_row.attribute_mut_or_insert_default::<DAffine2>(ATTR_TRANSFORM).translation = transformed_point;
				result_list.push(generated_row);
			}
		}
	}

	Ok(result_list)
}
