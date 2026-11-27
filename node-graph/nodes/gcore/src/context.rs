use core_types::list::{Item, List, NodeIdPath};
use core_types::transform::Footprint;
use core_types::{ATTR_EDITOR_FOOTPRINT_AREAS, ATTR_TRANSFORM, CloneVarArgs, Color, Context, ExtractAll, ExtractVarArgs, InjectFootprint, Interrupt, OwnedContextImpl};
use core_types::{Ctx, ExtractIndex, ExtractPosition};
use glam::{DAffine2, DVec2};
use graphic_types::vector_types::Gradient;
use graphic_types::{Artboard, Generator, Graphic, Vector};
use raster_types::{CPU, GPU, Raster};

#[derive(Debug, thiserror::Error)]
pub enum ReadContextError {
	#[error("No {0} to read: this node only works inside a loop over {0} items, like in a Map node")]
	NoLoopItem(&'static str),
	#[error("No position to read: this node only works inside a loop over points, like in a Repeat on Points node")]
	NoLoopPosition,
	#[error("Loop Level {level} is too high: the outermost loop around this node is level {outermost}")]
	LoopLevelTooHigh { level: i64, outermost: usize },
}

#[node_macro::node(category("Data: Context"), icon("NodeReadContext"), path(graphene_core::vector))]
fn read_graphic(ctx: impl Ctx + ExtractVarArgs) -> Result<Item<Graphic>, ReadContextError> {
	ctx.vararg(0).ok().and_then(|var_arg| var_arg.downcast_ref().cloned()).ok_or(ReadContextError::NoLoopItem("graphic"))
}

#[node_macro::node(category("Data: Context"), icon("NodeReadContext"), path(graphene_core::vector))]
fn read_vector(ctx: impl Ctx + ExtractVarArgs) -> Result<Item<Vector>, ReadContextError> {
	ctx.vararg(0).ok().and_then(|var_arg| var_arg.downcast_ref().cloned()).ok_or(ReadContextError::NoLoopItem("vector"))
}

#[node_macro::node(category("Data: Context"), icon("NodeReadContext"), path(graphene_core::vector))]
fn read_raster(ctx: impl Ctx + ExtractVarArgs) -> Result<Item<Raster<CPU>>, ReadContextError> {
	ctx.vararg(0).ok().and_then(|var_arg| var_arg.downcast_ref().cloned()).ok_or(ReadContextError::NoLoopItem("raster"))
}

#[node_macro::node(category("Data: Context"), icon("NodeReadContext"), path(graphene_core::vector))]
fn read_color(ctx: impl Ctx + ExtractVarArgs) -> Result<Item<Color>, ReadContextError> {
	ctx.vararg(0).ok().and_then(|var_arg| var_arg.downcast_ref().cloned()).ok_or(ReadContextError::NoLoopItem("color"))
}

#[node_macro::node(category("Data: Context"), icon("NodeReadContext"), path(graphene_core::vector))]
fn read_gradient(ctx: impl Ctx + ExtractVarArgs) -> Result<Item<Gradient>, ReadContextError> {
	ctx.vararg(0).ok().and_then(|var_arg| var_arg.downcast_ref().cloned()).ok_or(ReadContextError::NoLoopItem("gradient"))
}

/// Reads the current number from within a **Map** node's loop.
#[node_macro::node(category("Data: Context"), icon("NodeReadContext"))]
fn read_number(ctx: impl Ctx + ExtractVarArgs) -> Result<Item<f64>, ReadContextError> {
	let var_arg = ctx.vararg(0).ok();

	if let Some(item) = var_arg.and_then(|var_arg| var_arg.downcast_ref::<Item<f64>>()) {
		return Ok(item.clone());
	}

	// An integer list widens to f64, keeping the item's attributes
	if let Some(item) = var_arg.and_then(|var_arg| var_arg.downcast_ref::<Item<i64>>()) {
		let (element, attributes) = item.clone().into_parts();
		return Ok(Item::from_parts(element as f64, attributes));
	}

	Err(ReadContextError::NoLoopItem("number"))
}

#[node_macro::node(category("Data: Context"), icon("NodeReadPosition"), path(core_types::vector))]
fn read_position(
	ctx: impl Ctx + ExtractPosition,
	_primary: (),
	/// The number of nested loops to traverse outwards (from the innermost loop) to get the position from. The most upstream loop is level 0, and downstream loops add levels.
	///
	/// In programming terms: inside the double loop `i { j { ... } }`, *Loop Level* 0 = `j` and 1 = `i`. After inserting a third loop `k { ... }`, inside it, levels would be 0 = `k`, 1 = `j`, and 2 = `i`.
	#[hard(0..)]
	loop_level: Item<i64>,
) -> Result<Item<DVec2>, ReadContextError> {
	let positions = ctx.try_position().ok_or(ReadContextError::NoLoopPosition)?;
	Ok(Item::new_from_element(at_loop_level(positions, *loop_level.element())?))
}

// TODO: Return i64 instead of f64 once automatic type conversion is implemented for nodes with generic type inputs, so an integer output doesn't wall this index off from the generic math nodes.
// TODO: (Currently automatic type conversion only works for concrete types, via the Graphene preprocessor and not the full Graphene type system.)
/// Produces the index of the current iteration of a loop by reading from the evaluation context, which is supplied by downstream nodes such as *Repeat*.
///
/// Nested loops can enable 2D or higher-dimensional iteration by using the *Loop Level* parameter to read the index from outer levels of loops.
///
/// Outside of any loop, the index is 0.
#[node_macro::node(category("Data: Context"), icon("NodeReadIndex"), path(core_types::vector))]
fn read_index(
	ctx: impl Ctx + ExtractIndex,
	_primary: (),
	/// The number of nested loops to traverse outwards (from the innermost loop) to get the index from. The most upstream loop is level 0, and downstream loops add levels.
	///
	/// In programming terms: inside the double loop `i { j { ... } }`, *Loop Level* 0 = `j` and 1 = `i`. After inserting a third loop `k { ... }`, inside it, levels would be 0 = `k`, 1 = `j`, and 2 = `i`.
	#[hard(0..)]
	loop_level: Item<i64>,
) -> Result<Item<f64>, ReadContextError> {
	// The graph outside any loop runs once, as index 0
	let Some(indices) = ctx.try_index() else { return Ok(Item::new_from_element(0.)) };

	Ok(Item::new_from_element(at_loop_level(indices, *loop_level.element())? as f64))
}

/// Evaluates the content with a fixed footprint in place of the one requested by the renderer, so the result no longer depends on the view.
#[node_macro::node(category("Data: Context"))]
fn bake_footprint<T>(
	ctx: impl Ctx + ExtractAll + CloneVarArgs + InjectFootprint,
	#[implementations(
		Context -> Item<bool>,
		Context -> Item<i64>,
		Context -> Item<f64>,
		Context -> Item<String>,
		Context -> Item<DAffine2>,
		Context -> Item<Footprint>,
		Context -> Item<DVec2>,
		Context -> Item<Vector>,
		Context -> Item<Graphic>,
		Context -> Item<Raster<CPU>>,
		Context -> Item<Raster<GPU>>,
		Context -> Item<Color>,
		Context -> Item<Gradient>,
		Context -> Item<Generator>,
		Context -> Item<Artboard>,
		Context -> List<String>,
		Context -> List<f64>,
		Context -> List<DVec2>,
		Context -> List<Vector>,
		Context -> List<Graphic>,
		Context -> List<Raster<CPU>>,
		Context -> List<Raster<GPU>>,
		Context -> List<Color>,
		Context -> List<Gradient>,
		Context -> List<Generator>,
		Context -> List<Artboard>,
	)]
	value: impl Node<Context<'static>, Output = T>,
	/// The area of the document and the resolution the content is evaluated at.
	#[default(0., 0., 1000., 1000.)]
	footprint: Item<Footprint>,
	#[node_path] node_path: Item<NodeIdPath>,
) -> Result<T, Interrupt>
where
	T: RecordFootprintArea,
{
	let area = footprint.into_element();
	let unit_square_to_pixels = DAffine2::from_scale(area.resolution.as_dvec2());
	let footprint = Footprint {
		transform: unit_square_to_pixels * area.transform.inverse(),
		..area
	};

	let new_context = OwnedContextImpl::from(ctx).with_footprint(footprint);
	let mut content = value.eval(Some(new_context.into()))?;

	content.record_footprint_area(node_path.element(), area.transform);
	Ok(content)
}

/// Content whose items can record the footprint areas they were evaluated in.
pub trait RecordFootprintArea {
	/// Appends the area of the node at `node_path` to each item. `area` maps the unit square into the space the items are placed in.
	fn record_footprint_area(&mut self, node_path: &NodeIdPath, area: DAffine2);
}

impl<T> RecordFootprintArea for Item<T> {
	fn record_footprint_area(&mut self, node_path: &NodeIdPath, area: DAffine2) {
		let item_transform: DAffine2 = self.attribute_cloned_or_default(ATTR_TRANSFORM);
		let Some(footprint_area) = footprint_area_row(node_path, item_transform, area) else { return };

		self.attribute_mut_or_insert_default::<List<NodeIdPath>>(ATTR_EDITOR_FOOTPRINT_AREAS).push(footprint_area);
	}
}

impl<T> RecordFootprintArea for List<T> {
	fn record_footprint_area(&mut self, node_path: &NodeIdPath, area: DAffine2) {
		for index in 0..self.len() {
			let item_transform: DAffine2 = self.attribute_cloned_or_default(ATTR_TRANSFORM, index);
			let Some(footprint_area) = footprint_area_row(node_path, item_transform, area) else { continue };

			self.with_attribute_mut_or_default::<List<NodeIdPath>, _, _>(ATTR_EDITOR_FOOTPRINT_AREAS, index, |footprint_areas| footprint_areas.push(footprint_area));
		}
	}
}

/// The row recording the area relative to the item, so it follows the transforms later applied to the item.
fn footprint_area_row(node_path: &NodeIdPath, item_transform: DAffine2, area: DAffine2) -> Option<Item<NodeIdPath>> {
	if item_transform.matrix2.determinant() == 0. {
		return None;
	}

	Some(Item::new_from_element(node_path.clone()).with_attribute(ATTR_TRANSFORM, item_transform.inverse() * area))
}

/// The value of the loop `level` loops out from the innermost one, given the loops' values from the innermost outwards.
fn at_loop_level<T>(levels: impl Iterator<Item = T>, level: i64) -> Result<T, ReadContextError> {
	let levels: Vec<T> = levels.collect();
	let outermost = levels.len().saturating_sub(1);

	usize::try_from(level)
		.ok()
		.and_then(|level| levels.into_iter().nth(level))
		.ok_or(ReadContextError::LoopLevelTooHigh { level, outermost })
}

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::OwnedContextImpl;

	#[test]
	fn read_index_reads_each_loop_level_and_fails_past_the_outermost() {
		let nested = OwnedContextImpl::default().with_index(4).with_index(7).into_context();
		let level = |level: i64| read_index(nested.clone(), (), Item::new_from_element(level)).map(|index| *index.element());

		assert_eq!(level(0).unwrap(), 7.);
		assert_eq!(level(1).unwrap(), 4.);
		assert_eq!(level(2).unwrap_err().to_string(), "Loop Level 2 is too high: the outermost loop around this node is level 1");
		assert!(matches!(level(i64::MAX), Err(ReadContextError::LoopLevelTooHigh { .. })));
	}

	#[test]
	fn read_index_outside_a_loop_is_zero() {
		let outside = |level: i64| read_index(OwnedContextImpl::default().into_context(), (), Item::new_from_element(level)).map(|index| *index.element());

		assert_eq!(outside(0).unwrap(), 0.);
		assert_eq!(outside(3).unwrap(), 0.);
	}

	#[test]
	fn read_number_outside_a_loop_fails() {
		let error = read_number(OwnedContextImpl::default().into_context()).unwrap_err();
		assert_eq!(error.to_string(), "No number to read: this node only works inside a loop over number items, like in a Map node");
	}
}
