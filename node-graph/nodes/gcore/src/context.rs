use core_types::gpoll::{Extent, GPoll, GraphError, Interrupt};
use core_types::list::List;
use core_types::{Color, ExtractVarArgs};
use core_types::{Ctx, ExtractIndex, ExtractIndices, ExtractPosition};
use glam::DVec2;
use graphic_types::vector_types::GradientStops;
use graphic_types::{Graphic, Vector};
use raster_types::{CPU, Raster};

#[node_macro::node(category("Context"), path(graphene_core::vector))]
fn read_graphic(ctx: impl Ctx + ExtractVarArgs) -> List<Graphic> {
	let Ok(var_arg) = ctx.vararg(0) else { return Default::default() };
	let var_arg = var_arg as &dyn std::any::Any;

	var_arg.downcast_ref().cloned().unwrap_or_default()
}

#[node_macro::node(category("Context"), path(graphene_core::vector))]
fn read_vector(ctx: impl Ctx + ExtractVarArgs) -> List<Vector> {
	let Ok(var_arg) = ctx.vararg(0) else { return Default::default() };
	let var_arg = var_arg as &dyn std::any::Any;

	var_arg.downcast_ref().cloned().unwrap_or_default()
}

#[node_macro::node(category("Context"), path(graphene_core::vector))]
fn read_raster(ctx: impl Ctx + ExtractVarArgs) -> List<Raster<CPU>> {
	let Ok(var_arg) = ctx.vararg(0) else { return Default::default() };
	let var_arg = var_arg as &dyn std::any::Any;

	var_arg.downcast_ref().cloned().unwrap_or_default()
}

#[node_macro::node(category("Context"), path(graphene_core::vector))]
fn read_color(ctx: impl Ctx + ExtractVarArgs) -> List<Color> {
	let Ok(var_arg) = ctx.vararg(0) else { return Default::default() };
	let var_arg = var_arg as &dyn std::any::Any;

	var_arg.downcast_ref().cloned().unwrap_or_default()
}

#[node_macro::node(category("Context"), path(graphene_core::vector))]
fn read_gradient(ctx: impl Ctx + ExtractVarArgs) -> List<GradientStops> {
	let Ok(var_arg) = ctx.vararg(0) else { return Default::default() };
	let var_arg = var_arg as &dyn std::any::Any;

	var_arg.downcast_ref().cloned().unwrap_or_default()
}

/// The mapped row riding as vararg 0, in the production single-item shape.
fn vararg_list<'a, T: 'static>(ctx: &'a impl ExtractVarArgs) -> Option<&'a List<T>> {
	let arg = ctx.vararg(0).ok()?;
	(arg as &dyn std::any::Any).downcast_ref::<List<T>>()
}

/// Lanes of a leveled vararg source: one per item, none without a row,
/// matching the legacy empty-list return.
fn vararg_lanes<T: 'static>(ctx: &impl ExtractVarArgs, level: u8) -> GPoll<Extent> {
	match level {
		0 => GPoll::Final(Extent::Exactly(vararg_list::<T>(ctx).map_or(0, List::len))),
		_ => GPoll::Final(Extent::Exactly(1)),
	}
}

fn vararg_element<T: Clone + 'static>(ctx: &(impl ExtractVarArgs + ExtractIndex)) -> Result<T, Interrupt> {
	vararg_list::<T>(ctx)
		.and_then(|list| list.element(ctx.index() as usize))
		.cloned()
		.ok_or_else(|| GraphError::new("vararg row addressed past its items").into())
}

/// Rank-model vararg source: the mapped row's items as lanes, elements only.
#[node_macro::node(category("Test"), extent_raw(read_graphic_row_extent))]
pub fn read_graphic_row(ctx: impl Ctx + ExtractVarArgs + ExtractIndex) -> Result<IList<Graphic>, Interrupt> {
	vararg_element(ctx)
}

fn read_graphic_row_extent<C: Ctx + ExtractVarArgs>(_: &ReadGraphicRowNode, ctx: &C, level: u8) -> GPoll<Extent> {
	vararg_lanes::<Graphic>(ctx, level)
}

/// Rank-model vararg source: the mapped row's items as lanes, elements only.
#[node_macro::node(category("Test"), extent_raw(read_vector_row_extent))]
pub fn read_vector_row(ctx: impl Ctx + ExtractVarArgs + ExtractIndex) -> Result<IList<Vector>, Interrupt> {
	vararg_element(ctx)
}

fn read_vector_row_extent<C: Ctx + ExtractVarArgs>(_: &ReadVectorRowNode, ctx: &C, level: u8) -> GPoll<Extent> {
	vararg_lanes::<Vector>(ctx, level)
}

/// Rank-model vararg source: the mapped row's items as lanes, elements only.
#[node_macro::node(category("Test"), extent_raw(read_raster_row_extent))]
pub fn read_raster_row(ctx: impl Ctx + ExtractVarArgs + ExtractIndex) -> Result<IList<Raster<CPU>>, Interrupt> {
	vararg_element(ctx)
}

fn read_raster_row_extent<C: Ctx + ExtractVarArgs>(_: &ReadRasterRowNode, ctx: &C, level: u8) -> GPoll<Extent> {
	vararg_lanes::<Raster<CPU>>(ctx, level)
}

/// Rank-model vararg source: the mapped row's items as lanes, elements only.
#[node_macro::node(category("Test"), extent_raw(read_color_row_extent))]
pub fn read_color_row(ctx: impl Ctx + ExtractVarArgs + ExtractIndex) -> Result<IList<Color>, Interrupt> {
	vararg_element(ctx)
}

fn read_color_row_extent<C: Ctx + ExtractVarArgs>(_: &ReadColorRowNode, ctx: &C, level: u8) -> GPoll<Extent> {
	vararg_lanes::<Color>(ctx, level)
}

/// Rank-model vararg source: the mapped row's items as lanes, elements only.
#[node_macro::node(category("Test"), extent_raw(read_gradient_row_extent))]
pub fn read_gradient_row(ctx: impl Ctx + ExtractVarArgs + ExtractIndex) -> Result<IList<GradientStops>, Interrupt> {
	vararg_element(ctx)
}

fn read_gradient_row_extent<C: Ctx + ExtractVarArgs>(_: &ReadGradientRowNode, ctx: &C, level: u8) -> GPoll<Extent> {
	vararg_lanes::<GradientStops>(ctx, level)
}

#[node_macro::node(category("Context"), path(core_types::vector))]
fn read_position(
	ctx: impl Ctx + ExtractPosition,
	_primary: (),
	/// The number of nested loops to traverse outwards (from the innermost loop) to get the position from. The most upstream loop is level 0, and downstream loops add levels.
	///
	/// In programming terms: inside the double loop `i { j { ... } }`, *Loop Level* 0 = `j` and 1 = `i`. After inserting a third loop `k { ... }`, inside it, levels would be 0 = `k`, 1 = `j`, and 2 = `i`.
	loop_level: u32,
) -> DVec2 {
	ctx.try_position().and_then(|mut iter| iter.nth(loop_level as usize).or_else(|| iter.last())).unwrap_or(DVec2::ZERO)
}

// TODO: Return u32, u64, or usize instead of f64 after #1621 is resolved and has allowed us to implement automatic type conversion in the node graph for nodes with generic type inputs.
// TODO: (Currently automatic type conversion only works for concrete types, via the Graphene preprocessor and not the full Graphene type system.)
/// Produces the index of the current iteration of a loop by reading from the evaluation context, which is supplied by downstream nodes such as *Repeat*.
///
/// Nested loops can enable 2D or higher-dimensional iteration by using the *Loop Level* parameter to read the index from outer levels of loops.
#[node_macro::node(category("Context"), path(core_types::vector))]
fn read_index(
	// `loop_level` is a runtime input, so no level is statically known and the
	// whole chain has to survive nullification.
	ctx: impl Ctx + ExtractIndices,
	_primary: (),
	/// The number of nested loops to traverse outwards (from the innermost loop) to get the index from. The most upstream loop is level 0, and downstream loops add levels.
	///
	/// In programming terms: inside the double loop `i { j { ... } }`, *Loop Level* 0 = `j` and 1 = `i`. After inserting a third loop `k { ... }`, inside it, levels would be 0 = `k`, 1 = `j`, and 2 = `i`.
	loop_level: u32,
) -> f64 {
	// The chain's innermost entry is the consuming wire's own lane from the
	// decompose-and-promote split; the loops the reader counts sit above it.
	ctx.try_index().and_then(|mut iter| iter.nth(loop_level as usize + 1).or_else(|| iter.last())).unwrap_or(0) as f64
}
