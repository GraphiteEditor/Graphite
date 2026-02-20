use core_types::ExtractVarArgs;
use core_types::table::Table;
use core_types::{Ctx, ExtractIndex, ExtractPosition};
use glam::DVec2;
use graphic_types::Vector;

// TODO: Call this "Read Context" once it's fully generic
#[node_macro::node(category("Context"), path(graphene_core::vector))]
fn read_vector(ctx: impl Ctx + ExtractVarArgs) -> Table<Vector> {
	let Ok(var_arg) = ctx.vararg(0) else { return Default::default() };
	let var_arg = var_arg as &dyn std::any::Any;

	var_arg.downcast_ref().cloned().unwrap_or_default()
}

#[node_macro::node(category("Context"), path(core_types::vector))]
async fn read_position(
	ctx: impl Ctx + ExtractPosition,
	_primary: (),
	/// The number of nested loops to traverse outwards (from the innermost loop) to get the position from. The most upstream loop is level 0, and downstream loops add levels.
	///
	/// In programming terms: inside the double loop `i { j { ... } }`, *Loop Level* 0 = `j` and 1 = `i`. After inserting a third loop `k { ... }`, inside it, levels would be 0 = `k`, 1 = `j`, and 2 = `i`.
	loop_level: u32,
) -> DVec2 {
	let Some(position_iter) = ctx.try_position() else { return DVec2::ZERO };
	let mut last = DVec2::ZERO;
	for (i, position) in position_iter.enumerate() {
		if i == loop_level as usize {
			return position;
		}
		last = position;
	}

	last
}

// TODO: Return u32, u64, or usize instead of f64 after #1621 is resolved and has allowed us to implement automatic type conversion in the node graph for nodes with generic type inputs.
// TODO: (Currently automatic type conversion only works for concrete types, via the Graphene preprocessor and not the full Graphene type system.)
/// Produces the index of the current iteration of a loop by reading from the evaluation context, which is supplied by downstream nodes such as *Instance Repeat*.
///
/// Nested loops can enable 2D or higher-dimensional iteration by using the *Loop Level* parameter to read the index from outer levels of loops.
#[node_macro::node(category("Context"), path(core_types::vector))]
async fn read_index(
	ctx: impl Ctx + ExtractIndex,
	_primary: (),
	/// The number of nested loops to traverse outwards (from the innermost loop) to get the index from. The most upstream loop is level 0, and downstream loops add levels.
	///
	/// In programming terms: inside the double loop `i { j { ... } }`, *Loop Level* 0 = `j` and 1 = `i`. After inserting a third loop `k { ... }`, inside it, levels would be 0 = `k`, 1 = `j`, and 2 = `i`.
	loop_level: u32,
) -> f64 {
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
