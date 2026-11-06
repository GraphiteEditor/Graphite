use core_types::list::Item;
use core_types::{Color, ExtractVarArgs};
use core_types::{Ctx, ExtractIndex, ExtractPosition};
use glam::DVec2;
use graphic_types::vector_types::Gradient;
use graphic_types::{Graphic, Vector};
use raster_types::{CPU, Raster};

#[derive(Debug, thiserror::Error)]
pub enum ReadContextError {
	#[error("No {0} to read: this node only works inside a loop over {0} items, like in a Map node")]
	NoLoopItem(&'static str),
	#[error("No loop index to read: this node only works inside a loop, like in a Repeat or Map node")]
	NoLoopIndex,
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
	let indices = ctx.try_index().ok_or(ReadContextError::NoLoopIndex)?;
	Ok(Item::new_from_element(at_loop_level(indices, *loop_level.element())? as f64))
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

		let outside = read_index(OwnedContextImpl::default().into_context(), (), Item::new_from_element(0));
		assert!(matches!(outside, Err(ReadContextError::NoLoopIndex)));
	}

	#[test]
	fn read_number_outside_a_loop_fails() {
		let error = read_number(OwnedContextImpl::default().into_context()).unwrap_err();
		assert_eq!(error.to_string(), "No number to read: this node only works inside a loop over number items, like in a Map node");
	}
}
