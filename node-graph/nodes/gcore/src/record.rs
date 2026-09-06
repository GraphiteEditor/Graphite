//! Pilot record nodes exercising the macro's record-tier attribute io:
//! per-input tuple reads resolved against each input, offset writes,
//! `RemoveAttr` layout subtraction, the ElToken byte-carry for passthrough
//! elements, and the `_: ()` no-carrier form. These are the flat-wave law
//! tests; the node forms are the production authoring surface, and the
//! wiring is by hand until the compiler pass constructs layouts.

use core_types::Ctx;
use core_types::attribute::{Attr, EditorLayerPath, Opacity, OwnedAttr, RemoveAttr, Transform};
use core_types::context::{DeriveCtx, ExtractIndex, IndexLink, InjectIndex, ModifyIndex};
use core_types::extent::{ExtentIn, LevelIn, ListIn, ValueIn};
use core_types::gpoll::{ErrorKind, Extent, GPoll, GraphError, Interrupt, Level};
use core_types::node::Lane;
use core_types::uuid::NodeId;
use glam::DAffine2;

core_types::attribute! {
	/// Test-only measured length of an element.
	pub Length("length"): f64;
	/// Test-only label parked in the arena by its writer.
	pub Label("label"): &str;
}

#[node_macro::node(category("Test"))]
fn multiply_opacity(_: impl Ctx, (element, opacity): (f64, Attr<Opacity>), factor: f64) -> (f64, Attr<Opacity>) {
	(element, Attr(*opacity * factor))
}

#[node_macro::node(category("Test"))]
fn measure(_: impl Ctx, element: f64) -> (f64, Attr<Length>) {
	(element, Attr(element.abs()))
}

#[node_macro::node(category("Test"))]
fn shade(_: impl Ctx, (element, opacity): (f64, Attr<Opacity>)) -> f64 {
	element * *opacity
}

#[node_macro::node(category("Test"))]
fn checked_multiply_opacity(_: impl Ctx, (element, opacity): (f64, Attr<Opacity>), factor: f64) -> Result<(f64, Attr<Opacity>), Interrupt> {
	if factor < 0. {
		return Err(GraphError::new("negative factor").into());
	}
	Ok((element, Attr(*opacity * factor)))
}

#[node_macro::node(category("Test"))]
fn scale(_: impl Ctx, (element, opacity): (f64, Attr<Opacity>), factor: &f64) -> (f64, Attr<Opacity>) {
	(element * *factor, Attr(*opacity))
}

#[node_macro::node(category("Test"))]
fn fade<T>(_: impl Ctx, (element, opacity): (T, Attr<Opacity>), factor: f64) -> (T, Attr<Opacity>) {
	(element, Attr(*opacity * factor))
}

/// Test-only structure creator: the `IList` return pushes one rank level and
/// writes a per-copy opacity indexed by the copy's own index.
#[node_macro::node(category("Test"), extent(repeat_opacity_extent))]
fn repeat_opacity(ctx: impl Ctx + ExtractIndex, element: f64, count: u32) -> IList<(f64, Attr<Opacity>)> {
	debug_assert!(ctx.index() < count as u64, "repeat addressed past its copy count");
	emit(element, Attr(ctx.index() as f64))
}

#[node_macro::node(category("Test"))]
fn sum(_: impl Ctx, items: IList<f64>) -> f64 {
	items.into_iter().sum()
}

#[node_macro::node(category("Test"))]
fn sum_nested(_: impl Ctx, items: IList<IList<f64>>) -> f64 {
	items.into_iter().sum()
}

/// The pushed level's extent is the copy count; other levels forward to the carrier.
fn repeat_opacity_extent(element: ExtentIn<'_>, count: ValueIn<'_, u32>, level: LevelIn) -> GPoll<Extent> {
	match level.pushed() {
		true => count.get().map(|count| Extent::Exactly(count as usize)),
		false => element.at(level),
	}
}

/// Generic structure creator: evaluates the lazy content once per copy with the
/// copy's index pushed in, producing a rank level of `count` copies.
#[node_macro::node(category("Test"), extent(repeat_extent))]
fn repeat<T>(
	ctx: impl Ctx + DeriveCtx + ExtractIndex,
	content: impl Node<Context<'_>, Output = T>,
	#[default(1)]
	#[hard(1..)]
	count: u32,
	reverse: bool,
) -> Result<IList<T>, Interrupt> {
	let inner = content.inner_extent(ctx)?;
	let (copy, rest) = ctx.split_innermost(inner);
	if copy >= count as u64 {
		return Err(GraphError::past_end().into());
	}
	let copy = match reverse {
		true => count as u64 - 1 - copy,
		false => copy,
	};
	let mut frame = IndexLink { index: 0, outer: None };
	content.eval(&ctx.push_level(&mut frame, copy, rest))
}

/// The pushed level's extent is the copy count; inner levels forward to the
/// content, whose extent is taken uniform across copies (queried at copy 0).
fn repeat_extent(content: ExtentIn<'_>, count: ValueIn<'_, u32>, _reverse: ValueIn<'_, bool>, level: LevelIn) -> GPoll<Extent> {
	match level.pushed() {
		true => count.get().map(|count| Extent::Exactly(count as usize)),
		false => content.at(level),
	}
}

/// Test-only lazy-carrier creator: each copy evaluates the content at its own
/// index and re-scales the row's opacity by the copy number.
#[node_macro::node(category("Test"), extent(repeat_faded_extent))]
fn repeat_faded<T>(ctx: impl Ctx + DeriveCtx + ExtractIndex, content: impl Node<Context<'_>, Output = (T, Attr<Opacity>)>, count: u32) -> Result<IList<(T, Attr<Opacity>)>, Interrupt> {
	let inner = content.inner_extent(ctx)?;
	let (copy, rest) = ctx.split_innermost(inner);
	if copy >= count as u64 {
		return Err(GraphError::past_end().into());
	}
	let mut frame = IndexLink { index: 0, outer: None };
	let (element, opacity) = content.eval(&ctx.push_level(&mut frame, copy, rest))?;
	Ok(emit(element, Attr(*opacity * (copy + 1) as f64)))
}

/// The pushed level's extent is the copy count; inner levels forward to the
/// content, whose extent is taken uniform across copies (queried at copy 0).
fn repeat_faded_extent(content: ExtentIn<'_>, count: ValueIn<'_, u32>, level: LevelIn) -> GPoll<Extent> {
	match level.pushed() {
		true => count.get().map(|count| Extent::Exactly(count as usize)),
		false => content.at(level),
	}
}

/// Rank-model Extend: the output's top level is `base`'s lanes followed by
/// `new`'s, each side evaluated within its own index range.
#[node_macro::node(category("Test"), extent(extend_extent))]
fn extend<T>(ctx: impl Ctx + ExtractIndex + InjectIndex + Copy, base: impl Node<Context<'_>, Output = T>, new: impl Node<Context<'_>, Output = T>) -> Result<T, Interrupt> {
	let split = match base.extent(ctx, Level::Total) {
		GPoll::Final(Extent::Exactly(count)) => count as u64,
		// A scalar side joins the concat as a single lane, per `Extent::sum`.
		GPoll::Final(Extent::Free) => 1,
		GPoll::Pending => return Err(Interrupt::Pending),
		_ => return Err(GraphError::new("extend over a non-exact base extent").into()),
	};
	let lane = ctx.index();
	match lane < split {
		true => base.eval(ctx),
		false => {
			let mut shifted = *ctx;
			shifted.set_index(lane - split);
			new.eval(&shifted)
		}
	}
}

/// The top level sums both sides; inner levels must agree (rectangular), a
/// free side defers to the other.
fn extend_extent(base: ExtentIn<'_>, new: ExtentIn<'_>, level: LevelIn) -> GPoll<Extent> {
	match level.top() {
		true => Extent::sum(base.at(level), new.at(level)),
		false => base.at(level).zip(new.at(level)).and_then(|extents| match extents {
			(Extent::Free, other) | (other, Extent::Free) => GPoll::Final(other),
			(base, new) if base == new => GPoll::Final(base),
			_ => GPoll::error("extend inner extents differ"),
		}),
	}
}

/// The layer-path stamp: writes the owning layer's document node path on each lane.
#[node_macro::node(category("Test"))]
fn stamp_layer_path<'e, T>(ctx: impl Ctx + ExtractArena<'e>, element: T, path: Vec<NodeId>) -> Result<(T, Attr<'e, EditorLayerPath>), Interrupt> {
	let (parked, _) = ctx.arena().alloc(path).ok_or(GraphError {
		kind: ErrorKind::ArenaExhausted,
		trace: Vec::new(),
	})?;
	Ok((element, Attr(parked.as_slice())))
}

/// Resolves a signed index over `total` lanes: negatives count from the end,
/// out of range resolves to nothing.
fn resolve_index(index: f64, total: u64) -> Option<u64> {
	let index = index as i64;
	match index < 0 {
		true => total.checked_sub(index.unsigned_abs()),
		false => ((index as u64) < total).then_some(index as u64),
	}
}

/// Rank-model Omit Element: the top level shrinks by one; lanes at or past
/// the omitted index read one lane further. An out-of-range index passes the
/// level through unchanged.
#[node_macro::node(category("Test"), extent(omit_element_extent))]
fn omit_element<T>(ctx: impl Ctx + ModifyIndex + Copy, content: impl Node<Context<'_>, Output = T>, index: f64) -> Result<T, Interrupt> {
	let total = match content.extent(ctx, Level::Total) {
		GPoll::Final(Extent::Exactly(count)) => count as u64,
		GPoll::Pending => return Err(Interrupt::Pending),
		_ => return Err(GraphError::new("omit over a non-exact extent").into()),
	};
	let lane = ctx.index();
	let source = match resolve_index(index, total) {
		Some(omitted) if lane >= omitted => lane + 1,
		_ => lane,
	};
	let mut shifted = *ctx;
	shifted.set_index(source);
	content.eval(&shifted)
}

fn omit_element_extent(content: ExtentIn<'_>, index: ValueIn<'_, f64>, level: LevelIn) -> GPoll<Extent> {
	match level.top() {
		true => index.get().zip(content.at(level)).map(|(index, extent)| match extent {
			Extent::Exactly(count) if resolve_index(index, count as u64).is_some() => Extent::Exactly(count - 1),
			extent => extent,
		}),
		false => content.at(level),
	}
}

/// Rank-model Index Elements: a one-lane level holding the item at the index
/// with its attributes, or an empty level when the index is out of range.
#[node_macro::node(category("Test"), extent(index_elements_extent))]
fn index_elements<T>(ctx: impl Ctx + ModifyIndex + Copy, content: impl Node<Context<'_>, Output = T>, index: f64) -> Result<T, Interrupt> {
	let total = match content.extent(ctx, Level::Total) {
		GPoll::Final(Extent::Exactly(count)) => count as u64,
		GPoll::Pending => return Err(Interrupt::Pending),
		_ => return Err(GraphError::new("index elements over a non-exact extent").into()),
	};
	let Some(source) = resolve_index(index, total) else {
		return Err(GraphError::new("index elements addressed its empty selection").into());
	};
	let mut shifted = *ctx;
	shifted.set_index(source);
	content.eval(&shifted)
}

fn index_elements_extent(content: ExtentIn<'_>, index: ValueIn<'_, f64>, level: LevelIn) -> GPoll<Extent> {
	match level.top() {
		true => index.get().zip(content.at(level)).map(|(index, extent)| match extent {
			Extent::Exactly(count) => Extent::Exactly(resolve_index(index, count as u64).is_some() as usize),
			_ => Extent::Exactly(1),
		}),
		false => content.at(level),
	}
}

/// Rank-model Extract Element: the bare element at the index, or the element
/// type's default when the index is out of range.
#[node_macro::node(category("Test"))]
fn extract_element(_: impl Ctx + InjectIndex + Copy, list: IList<f64>, index: f64) -> f64 {
	resolve_index(index, list.len() as u64).map(|resolved| list.get(resolved as usize)).unwrap_or_default()
}

/// Rank-model Mirror kernel: the level holds the content's lanes followed by
/// reflected copies (or the reflected copies alone), each reflected transform
/// mirrored about the level's horizontal center.
#[node_macro::node(category("Test"), extent(mirror_extent))]
fn mirror(ctx: impl Ctx + ExtractIndex + InjectIndex + Copy, content: IList<f64>, keep_original: bool) -> Result<IList<(f64, Attr<Transform>)>, Interrupt> {
	let total = content.len() as u64;
	let lane = ctx.index();
	let (source, mirrored) = match (keep_original, lane < total) {
		(true, true) => (lane, false),
		(true, false) => (lane - total, true),
		(false, _) => (lane, true),
	};
	if source >= total {
		return Err(GraphError::new("mirror addressed past its copy count").into());
	}

	let (min, max) = (0..content.len()).fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), row| {
		let x = content.lane(row).attr::<Transform>().translation.x;
		(min.min(x), max.max(x))
	});
	let center = (min + max) / 2.;

	let element = content.get(source as usize);
	let mut transform: DAffine2 = content.lane(source as usize).attr::<Transform>();
	if mirrored {
		transform.translation.x = 2. * center - transform.translation.x;
	}
	Ok((element, Attr(transform)))
}

/// The level doubles when the originals are kept.
fn mirror_extent(content: ListIn<'_, f64>, keep_original: ValueIn<'_, bool>, level: LevelIn) -> GPoll<Extent> {
	match level.top() {
		true => keep_original.get().zip(content.get()).map(|(keep, content)| match keep {
			true => Extent::Exactly(2 * content.len()),
			false => Extent::Exactly(content.len()),
		}),
		false => GPoll::Final(Extent::Exactly(1)),
	}
}

/// Gather-carrier kernel: the level's lanes in reverse. Returning a `Lane`
/// copies that lane's whole record, so undeclared attributes ride along and
/// only the declared opacity is rewritten.
#[node_macro::node(category("Test"), extent(reverse_lanes_extent))]
fn reverse_lanes(ctx: impl Ctx + ExtractIndex + InjectIndex + Copy, content: IList<f64>, opacity: f64) -> Result<IList<(Lane<f64>, Attr<Opacity>)>, Interrupt> {
	let lane = ctx.innermost_index() as usize;
	if lane >= content.len() {
		return Err(GraphError::past_end().into());
	}
	Ok((content.lane(content.len() - 1 - lane), Attr(opacity)))
}

fn reverse_lanes_extent(content: ListIn<'_, f64>, _opacity: ValueIn<'_, f64>, level: LevelIn) -> GPoll<Extent> {
	match level.top() {
		true => content.total(),
		false => GPoll::Final(Extent::Exactly(1)),
	}
}

#[node_macro::node(category("Test"))]
fn source_opacity(_: impl Ctx, _: (), element: f64, opacity: f64) -> (f64, Attr<Opacity>) {
	(element, Attr(opacity))
}

#[node_macro::node(category("Test"))]
fn label<'e>(ctx: impl Ctx + ExtractArena<'e>, (element, label): (f64, Attr<Label>), text: String) -> Result<(f64, Attr<'e, Label>), Interrupt> {
	let joined = format!("{}{text}", *label);
	let (parked, _) = ctx.arena().alloc(joined).ok_or(GraphError {
		kind: ErrorKind::ArenaExhausted,
		trace: Vec::new(),
	})?;
	Ok((element, Attr(parked.as_str())))
}

#[node_macro::node(category("Test"))]
fn transfer_opacity(_: impl Ctx, (element, opacity): (f64, Attr<Opacity>), (other, other_opacity): (f64, Attr<Opacity>)) -> (f64, Attr<Opacity>) {
	(element + other, Attr(*opacity * *other_opacity))
}

#[node_macro::node(category("Test"))]
fn strip_opacity<T>(_: impl Ctx, element: T) -> (T, RemoveAttr<Opacity>) {
	(element, RemoveAttr::new())
}

#[node_macro::node(category("Test"))]
fn relength(_: impl Ctx, element: f64) -> (f64, RemoveAttr<Opacity>, Attr<Length>) {
	(element, RemoveAttr::new(), Attr(element * 2.))
}

#[node_macro::node(category("Test"))]
fn boost(_: impl Ctx, element: f64, factor: f64) -> f64 {
	element * factor
}

#[node_macro::node(category("Test"))]
fn boost_poll(_: impl Ctx, element: f64, factor: f64) -> core_types::gpoll::GPoll<f64> {
	core_types::gpoll::GPoll::Final(element * factor)
}

#[node_macro::node(category("Test"))]
fn offset(_: impl Ctx, element: f64, by: &f64) -> f64 {
	element + *by
}

#[node_macro::node(category("Test"))]
async fn double_async(_: impl Ctx, element: f64) -> f64 {
	element * 2.
}

/// Test-only writing async source over a carrier: the slot stores the kernel's
/// whole tuple and the per-eval lift writes it through the claim.
#[node_macro::node(category("Test"))]
async fn fade_async(_: impl Ctx, element: f64, opacity: f64) -> (f64, Attr<Opacity>) {
	(element * 2., Attr(opacity))
}

/// Test-only writing async source without a carrier: a fresh record per eval.
#[node_macro::node(category("Test"))]
async fn measure_async(_: impl Ctx, _: (), element: f64) -> (f64, Attr<Length>) {
	(element, Attr(element.abs()))
}

/// Test-only async source whose reference value crosses the future boundary
/// owned: the slot holds the deep copy and every lift parks it afresh.
#[node_macro::node(category("Test"))]
async fn tag_async(_: impl Ctx, _: (), element: f64, label: String) -> (f64, OwnedAttr<Label>) {
	(element, OwnedAttr::new(label.as_str()))
}

#[node_macro::node(category("Test"))]
fn fallback(ctx: impl Ctx, _: (), #[expose] content: impl Node<Context<'_>, Output = (f64, Attr<Opacity>)>, #[expose] alternate: impl Node<Context<'_>, Output = f64>) -> Result<f64, Interrupt> {
	let (element, opacity) = content.eval(ctx)?;
	Ok(if *opacity > 0. { element } else { alternate.eval(ctx)? })
}

#[node_macro::node(category("Test"))]
fn pick<T>(ctx: impl Ctx, take_second: bool, first: impl Node<Context<'_>, Output = T>, second: impl Node<Context<'_>, Output = T>) -> Result<T, Interrupt> {
	if take_second { second.eval(ctx) } else { first.eval(ctx) }
}

#[node_macro::node(category("Test"))]
fn hold_first<T>(ctx: impl Ctx, take_second: bool, first: impl Node<Context<'_>, Output = T>, second: impl Node<Context<'_>, Output = T>) -> Result<T, Interrupt> {
	let held = first.eval(ctx)?;
	let alt = second.eval(ctx)?;
	Ok(if take_second { alt } else { held })
}

#[node_macro::node(category("Test"))]
fn forward_record<T>(_: impl Ctx, element: T) -> T {
	element
}

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::SourceId;
	use core_types::arena::Arena;
	use core_types::attribute::Attribute as AttributeMarker;
	use core_types::context::{ContextImpl, EvalScope, ExtractArena};
	use core_types::gpoll::GPoll;
	use core_types::node::Node;
	use core_types::record::{FrameClaim, Layout, LiftedSource, Rec, RecordSource, Served};
	use core_types::value::ValueSource;

	struct RecordSourceNode<E> {
		layout: Layout,
		element: E,
		fields: Vec<(&'static str, f64)>,
		partial: bool,
	}

	impl<C, E: Copy + Send + Sync + dyn_any::StaticTypeSized + 'static> Node<C> for RecordSourceNode<E> {
		fn serve<'e, 'l>(&self, input: &C, slot: FrameClaim<'e, 'l>) -> GPoll<Served<'e>>
		where
			C: ExtractArena<ArenaRef = &'e Arena>,
		{
			let mut frame = slot;
			let arena = ExtractArena::arena(input);
			if frame.element(self.element, arena).is_none() {
				return GPoll::arena_exhausted();
			}
			for (name, field) in &self.fields {
				write_field_at(&mut frame, &self.layout, name, 0, *field);
			}
			// SAFETY: the writes above complete the record of this layout.
			let served = unsafe { frame.finish_served() };
			match self.partial {
				true => GPoll::Partial(served),
				false => GPoll::Final(served),
			}
		}

		fn layout(&self) -> &Layout {
			&self.layout
		}
	}

	struct LeveledSourceNode {
		layout: Layout,
		elements: Vec<f64>,
		field: Option<(&'static str, f64)>,
	}

	impl<C: ExtractIndex> Node<C> for LeveledSourceNode {
		fn serve<'e, 'l>(&self, input: &C, slot: FrameClaim<'e, 'l>) -> GPoll<Served<'e>>
		where
			C: ExtractArena<ArenaRef = &'e Arena>,
		{
			let element = self.elements[input.innermost_index() as usize % self.elements.len()];
			let mut frame = slot;
			let arena = ExtractArena::arena(input);
			if frame.element(element, arena).is_none() {
				return GPoll::arena_exhausted();
			}
			if let Some((name, value)) = self.field {
				write_field_at(&mut frame, &self.layout, name, 0, value);
			}
			// SAFETY: the writes above complete the record of this layout.
			GPoll::Final(unsafe { frame.finish_served() })
		}

		fn extent_at<'x>(&self, _input: &C, _level: u8, _frames: &core_types::record::Frames<'x>) -> GPoll<Extent>
		where
			C: ExtractArena<ArenaRef = &'x Arena>,
		{
			GPoll::Final(Extent::Exactly(self.elements.len()))
		}

		fn layout(&self) -> &Layout {
			&self.layout
		}
	}

	struct LeveledTransformSource {
		layout: Layout,
		rows: Vec<(f64, DAffine2)>,
	}

	impl<C: ExtractIndex> Node<C> for LeveledTransformSource {
		fn serve<'e, 'l>(&self, input: &C, slot: FrameClaim<'e, 'l>) -> GPoll<Served<'e>>
		where
			C: ExtractArena<ArenaRef = &'e Arena>,
		{
			let (element, transform) = self.rows[input.innermost_index() as usize % self.rows.len()];
			let mut frame = slot;
			let arena = ExtractArena::arena(input);
			if frame.element(element, arena).is_none() {
				return GPoll::arena_exhausted();
			}
			write_attr_at::<Transform>(&mut frame, &self.layout, transform);
			// SAFETY: the writes above complete the record of this layout.
			GPoll::Final(unsafe { frame.finish_served() })
		}

		fn extent_at<'x>(&self, _input: &C, _level: u8, _frames: &core_types::record::Frames<'x>) -> GPoll<Extent>
		where
			C: ExtractArena<ArenaRef = &'x Arena>,
		{
			GPoll::Final(Extent::Exactly(self.rows.len()))
		}

		fn layout(&self) -> &Layout {
			&self.layout
		}
	}

	/// A leveled source that keeps its count to itself: the extent is a lower
	/// bound and lanes past the data answer the past-end signal.
	struct DrainSourceNode {
		layout: Layout,
		count: usize,
	}

	impl<C: ExtractIndex> Node<C> for DrainSourceNode {
		fn serve<'e, 'l>(&self, input: &C, slot: FrameClaim<'e, 'l>) -> GPoll<Served<'e>>
		where
			C: ExtractArena<ArenaRef = &'e Arena>,
		{
			let lane = input.innermost_index();
			if lane >= self.count as u64 {
				return GPoll::past_end();
			}
			let mut frame = slot;
			let arena = ExtractArena::arena(input);
			if frame.element(lane as f64, arena).is_none() {
				return GPoll::arena_exhausted();
			}
			// SAFETY: the writes above complete the record of this layout.
			GPoll::Final(unsafe { frame.finish_served() })
		}

		fn extent_at<'x>(&self, _input: &C, _level: u8, _frames: &core_types::record::Frames<'x>) -> GPoll<Extent>
		where
			C: ExtractArena<ArenaRef = &'x Arena>,
		{
			GPoll::Final(Extent::AtLeast(0))
		}

		fn layout(&self) -> &Layout {
			&self.layout
		}
	}

	struct IndexSourceNode {
		layout: Layout,
	}

	impl<C: ExtractIndex + core_types::context::ExtractIndices> Node<C> for IndexSourceNode {
		fn serve<'e, 'l>(&self, input: &C, slot: FrameClaim<'e, 'l>) -> GPoll<Served<'e>>
		where
			C: ExtractArena<ArenaRef = &'e Arena>,
		{
			// Depth-0 content varying per copy: the enclosing (pushed) level's
			// index sits one link above the content's own innermost lane.
			let element = input.try_index().and_then(|mut indices| indices.nth(1)).unwrap_or(0) as f64;
			let mut frame = slot;
			let arena = ExtractArena::arena(input);
			if frame.element(element, arena).is_none() {
				return GPoll::arena_exhausted();
			}
			// SAFETY: the writes above complete the record of this layout.
			GPoll::Final(unsafe { frame.finish_served() })
		}

		fn layout(&self) -> &Layout {
			&self.layout
		}
	}

	/// Writes a field at the layout's resolved offset, the wiring-proven pairing
	/// a generated node performs.
	fn write_field_at<T: Copy + 'static>(frame: &mut FrameClaim<'_, '_>, layout: &Layout, name: &str, level: u8, value: T) {
		let field = layout
			.fields
			.iter()
			.find(|field| field.name == name && field.level == level)
			.expect("the layout carries the written field");
		assert_eq!(field.type_id, std::any::TypeId::of::<T>(), "the field was declared at this value type");
		// SAFETY: the offset is this layout's own, at the field's declared type.
		unsafe { frame.attr_at(field.offset, value) };
	}

	/// [`write_field_at`] for a census marker at level 0.
	fn write_attr_at<A: core_types::attribute::Attribute>(frame: &mut FrameClaim<'_, '_>, layout: &Layout, value: A::Value<'static>)
	where
		A::Value<'static>: Copy + 'static,
	{
		write_field_at(frame, layout, A::NAME, 0, value);
	}
	fn scope_fixture<'a>(generations: &'a [(SourceId, u64)], arena: &'a Arena) -> EvalScope<'a> {
		EvalScope::new(Some(0.5), None, None, generations, arena)
	}

	fn f64_layout(names: &[&'static str]) -> Layout {
		let writes: Vec<core_types::record::FieldWrite> = names
			.iter()
			.map(|name| core_types::record::FieldWrite {
				name,
				level: 0,
				size: 8,
				align: 8,
				type_id: std::any::TypeId::of::<f64>(),
				read_erased: <Opacity as AttributeMarker>::read_erased,
				repark: None,
				content_hash: None,
				content_eq: None,
			})
			.collect();
		Layout::default().with_writes(0, core_types::record::element_write::<f64>(), &writes)
	}

	fn leveled_f64_layout(names: &[&'static str]) -> Layout {
		let writes: Vec<core_types::record::FieldWrite> = names
			.iter()
			.map(|name| core_types::record::FieldWrite {
				name,
				level: 0,
				size: 8,
				align: 8,
				type_id: std::any::TypeId::of::<f64>(),
				read_erased: <Opacity as AttributeMarker>::read_erased,
				repark: None,
				content_hash: None,
				content_eq: None,
			})
			.collect();
		Layout::default().with_writes(1, core_types::record::element_write::<f64>(), &writes)
	}

	fn frames_for(layouts: &[&Layout]) -> core_types::record::Frames<'static> {
		core_types::record::test_frames(layouts.iter().map(|layout| layout.frame_bytes()).sum::<usize>().max(1 << 12))
	}

	fn install<N: Node<ContextImpl<'static>>>(mut node: N, meta: core_types::record::LayoutMeta, inputs: &[Option<&Layout>]) -> N {
		// The fixtures wire constants into every eager input, which the compiler
		// pass records as lane-invariant.
		let resolved = core_types::record::RecordLayout {
			lane_invariant: u32::MAX,
			..meta.resolve(inputs)
		};
		<N as Node<ContextImpl<'static>>>::set_layout(&mut node, resolved);
		node
	}

	fn install_flip<N: Node<ContextImpl<'static>>>(mut node: N, layout: &Layout) -> N {
		let bundle = core_types::record::RecordLayout {
			frame_bytes: layout.frame_bytes(),
			plan: Vec::new(),
			layout: layout.clone(),
			lane_invariant: u32::MAX,
		};
		<N as Node<ContextImpl<'static>>>::set_layout(&mut node, bundle);
		node
	}

	fn lifted_value<T: Clone + Send + Sync + core_types::StaticTypeSized + 'static>(value: T) -> (ValueSource<T>, Layout)
	where
		T::Static: Clone + Send + Sync,
	{
		let lift = ValueSource::new(value);
		let layout = Node::<ContextImpl>::layout(&lift).clone();
		(lift, layout)
	}

	fn bare_source(layout: &Layout, element: f64) -> RecordSourceNode<f64> {
		RecordSourceNode {
			layout: layout.clone(),
			element,
			fields: vec![],
			partial: false,
		}
	}

	#[test]
	fn creator_pushes_a_rank_level() {
		let base = f64_layout(&[]);
		let leveled = repeat_opacity_layout(&base);
		assert_eq!(leveled.depth, 1, "the IList return pushed one rank level above the depth-0 carrier");
		assert_eq!(repeat_opacity_layout_meta().fold(&[Some(&base)]), leveled, "the level_delta metadata folds to the same leveled layout");
	}

	#[test]
	fn creator_eval_indexes_the_copy() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);
		let head = ctx.index_head();
		let indexed = ctx.promoted(&head, 5);

		let base = f64_layout(&[]);
		let leveled = repeat_opacity_layout(&base);
		let frames = frames_for(&[&base, &leveled]);

		let node = install(
			RepeatOpacityNode::new(bare_source(&base, 7.), ValueSource::new(8u32), &base),
			repeat_opacity_layout_meta(),
			&[Some(&base)],
		);
		assert_eq!(Node::<ContextImpl>::layout(&node), &leveled);
		let GPoll::Final(served) = core_types::record::capture(&node, &indexed, &frames) else {
			panic!("expected a final record");
		};
		assert_eq!(served.element::<f64>(), 7.);
		// The written opacity is this copy's own index (the head of the chain).
		assert_eq!(served.attr::<Opacity>(), 5.);
	}

	#[test]
	fn creator_extent_reports_the_count() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);
		let base = f64_layout(&[]);
		let frames = frames_for(&[&base]);

		let node = install(
			RepeatOpacityNode::new(bare_source(&base, 7.), ValueSource::new(3u32), &base),
			repeat_opacity_layout_meta(),
			&[Some(&base)],
		);
		// The pushed level (0, the only level) reports the copy count.
		assert_eq!(node.extent_at(&ctx, 0, &frames), core_types::gpoll::GPoll::Final(core_types::gpoll::Extent::Exactly(3)));
	}

	#[test]
	fn generic_repeat_pushes_a_level_and_forwards_the_element() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let base = f64_layout(&[]);
		let (count_edge, count_layout) = lifted_value(3u32);
		let (reverse_edge, reverse_layout) = lifted_value(false);
		let frames = frames_for(&[&base, &count_layout]);

		let meta = core_types::record::LayoutMeta {
			sources: vec![0],
			reads: vec![],
			element: core_types::record::ElementSpec::Carried,
			writes: vec![],
			removes: vec![],
			level_delta: 1,
			folded: None,
		};
		let node = install(
			RepeatNode::new(RecordSource::new(bare_source(&base, 7.), &base, &base), count_edge, reverse_edge, &base, &count_layout, &reverse_layout),
			meta,
			&[Some(&base)],
		);
		let leveled = Node::<ContextImpl>::layout(&node).clone();
		assert_eq!(leveled.depth, 1, "the IList return pushed one rank level above the depth-0 content");
		assert_eq!(node.extent_at(&ctx, 0, &frames), GPoll::Final(core_types::gpoll::Extent::Exactly(3)));

		let GPoll::Final(served) = core_types::record::capture(&node, &ctx, &frames) else {
			panic!("expected a final record");
		};
		assert_eq!(served.element::<f64>(), 7., "the opaque generic element forwarded unchanged");
	}

	#[test]
	fn repeat_evaluates_content_at_each_copy_index() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let base = f64_layout(&[]);
		let (count_edge, count_layout) = lifted_value(4u32);
		let (reverse_edge, reverse_layout) = lifted_value(false);
		let frames = frames_for(&[&base, &count_layout]);

		let meta = core_types::record::LayoutMeta {
			sources: vec![0],
			reads: vec![],
			element: core_types::record::ElementSpec::Carried,
			writes: vec![],
			removes: vec![],
			level_delta: 1,
			folded: None,
		};
		let repeat = install(
			RepeatNode::new(
				RecordSource::new(IndexSourceNode { layout: base.clone() }, &base, &base),
				count_edge,
				reverse_edge,
				&base,
				&count_layout,
				&reverse_layout,
			),
			meta,
			&[Some(&base)],
		);
		let head = ctx.index_head();
		for copy in 0..4 {
			let lane = ctx.promoted(&head, copy);
			let GPoll::Final(served) = core_types::record::capture(&repeat, &lane, &frames) else {
				panic!("expected a final record");
			};
			// The copy evaluated its content at its own pushed index.
			assert_eq!(served.element::<f64>(), copy as f64);
		}
	}

	#[test]
	fn repeat_reverse_flips_the_copy_order() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let base = f64_layout(&[]);
		let (count_edge, count_layout) = lifted_value(4u32);
		let (reverse_edge, reverse_layout) = lifted_value(true);
		let frames = frames_for(&[&base, &count_layout, &reverse_layout]);

		let meta = core_types::record::LayoutMeta {
			sources: vec![0],
			reads: vec![],
			element: core_types::record::ElementSpec::Carried,
			writes: vec![],
			removes: vec![],
			level_delta: 1,
			folded: None,
		};
		let repeat = install(
			RepeatNode::new(
				RecordSource::new(IndexSourceNode { layout: base.clone() }, &base, &base),
				count_edge,
				reverse_edge,
				&base,
				&count_layout,
				&reverse_layout,
			),
			meta,
			&[Some(&base)],
		);
		let head = ctx.index_head();
		for copy in 0..4u64 {
			let lane = ctx.promoted(&head, copy);
			let GPoll::Final(served) = core_types::record::capture(&repeat, &lane, &frames) else {
				panic!("expected a final record");
			};
			// Reversed: copy `j` evaluates its content at index `count - 1 - j`.
			assert_eq!(served.element::<f64>(), (3 - copy) as f64);
		}
	}

	#[test]
	fn repeat_decomposes_the_flat_index_over_depth_one_content() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let base = f64_layout(&[]);
		let leveled_content = repeat_opacity_layout(&base);
		let (count_edge, count_layout) = lifted_value(2u32);
		let (reverse_edge, reverse_layout) = lifted_value(false);
		let frames = frames_for(&[&base, &leveled_content, &count_layout, &reverse_layout]);

		let content = install(
			RepeatOpacityNode::new(bare_source(&base, 7.), ValueSource::new(3u32), &base),
			repeat_opacity_layout_meta(),
			&[Some(&base)],
		);
		let meta = core_types::record::LayoutMeta {
			sources: vec![0],
			reads: vec![],
			element: core_types::record::ElementSpec::Carried,
			writes: vec![],
			removes: vec![],
			level_delta: 1,
			folded: None,
		};
		let repeat = install(
			RepeatNode::new(
				RecordSource::new(content, &leveled_content, &leveled_content),
				count_edge,
				reverse_edge,
				&leveled_content,
				&count_layout,
				&reverse_layout,
			),
			meta,
			&[Some(&leveled_content)],
		);
		let two_level = Node::<ContextImpl>::layout(&repeat).clone();
		assert_eq!(two_level.depth, 2, "the pushed level sits above the content's own level");
		assert_eq!(repeat.extent_at(&ctx, 1, &frames), GPoll::Final(Extent::Exactly(2)), "the pushed level's extent is the copy count");
		assert_eq!(repeat.extent_at(&ctx, 0, &frames), GPoll::Final(Extent::Exactly(3)), "the content's level forwards");

		let head = ctx.index_head();
		for flat in 0..6u64 {
			let lane = ctx.promoted(&head, flat);
			let GPoll::Final(served) = core_types::record::capture(&repeat, &lane, &frames) else {
				panic!("expected a final record");
			};
			assert_eq!(served.element::<f64>(), 7.);
			// The flat index decomposes: the content sees the remainder as its
			// own innermost lane, so its per-lane opacity is `flat % 3`.
			assert_eq!(served.attr::<Opacity>(), (flat % 3) as f64);
		}
	}

	#[test]
	fn lazy_carrier_reads_and_rewrites_the_attr_per_copy() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let base = f64_layout(&[Opacity::NAME]);
		let content = RecordSourceNode {
			layout: base.clone(),
			element: 7.,
			fields: vec![(Opacity::NAME, 0.5)],
			partial: false,
		};
		let frames = frames_for(&[&base]);

		let node = install(
			RepeatFadedNode::new(RecordSource::new(content, &base, &base), ValueSource::new(4u32), &base),
			repeat_faded_layout_meta(),
			&[Some(&base)],
		);
		let leveled = Node::<ContextImpl>::layout(&node).clone();
		assert_eq!(leveled.depth, 1, "the IList return pushed one rank level above the content");
		assert_eq!(node.extent_at(&ctx, 0, &frames), GPoll::Final(Extent::Exactly(4)));

		let head = ctx.index_head();
		for copy in 0..4u64 {
			let lane = ctx.promoted(&head, copy);
			let GPoll::Final(served) = core_types::record::capture(&node, &lane, &frames) else {
				panic!("expected a final record");
			};
			// The content row's element forwards; its opacity re-scales per copy.
			assert_eq!(served.element::<f64>(), 7.);
			assert_eq!(served.attr::<Opacity>(), 0.5 * (copy + 1) as f64);
		}
	}

	#[test]
	fn extend_concatenates_the_top_level_and_fills_the_union() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let base_layout = leveled_f64_layout(&[Opacity::NAME]);
		let new_layout = leveled_f64_layout(&[Length::NAME]);
		let union = Layout::union(&[&base_layout, &new_layout]);
		let frames = frames_for(&[&base_layout, &new_layout, &union]);

		let base = LeveledSourceNode {
			layout: base_layout.clone(),
			elements: vec![10., 11.],
			field: Some((Opacity::NAME, 0.5)),
		};
		let new = LeveledSourceNode {
			layout: new_layout.clone(),
			elements: vec![100., 101., 102.],
			field: Some((Length::NAME, 7.)),
		};
		let meta = core_types::record::LayoutMeta {
			sources: vec![0, 1],
			reads: vec![],
			element: core_types::record::ElementSpec::Carried,
			writes: vec![],
			removes: vec![],
			level_delta: 0,
			folded: None,
		};
		let node = install(
			ExtendNode::new(RecordSource::new(base, &base_layout, &union), RecordSource::new(new, &new_layout, &union), &union),
			meta,
			&[Some(&base_layout), Some(&new_layout)],
		);
		let out = Node::<ContextImpl>::layout(&node).clone();
		assert_eq!(out.depth, 1);
		assert_eq!(node.extent_at(&ctx, 0, &frames), GPoll::Final(Extent::Exactly(5)), "the top level sums both sides");

		let head = ctx.index_head();
		let expected = [10., 11., 100., 101., 102.];
		for (lane, &element) in expected.iter().enumerate() {
			let scoped = ctx.promoted(&head, lane as u64);
			let GPoll::Final(served) = core_types::record::capture(&node, &scoped, &frames) else {
				panic!("expected a final record");
			};
			assert_eq!(served.element::<f64>(), element);
			let opacity = served.attr::<Opacity>();
			let length = served.attr::<Length>();
			match lane < 2 {
				// The base side wrote its opacity; length fills from the census.
				true => assert_eq!((opacity, length), (0.5, 0.)),
				// The new side wrote its length; opacity fills from the census.
				false => assert_eq!((opacity, length), (1., 7.)),
			}
		}
	}

	#[test]
	fn extend_chains_associatively() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let a_layout = leveled_f64_layout(&[Opacity::NAME]);
		let b_layout = leveled_f64_layout(&[Length::NAME]);
		let c_layout = leveled_f64_layout(&[Opacity::NAME]);
		let ab = Layout::union(&[&a_layout, &b_layout]);
		let bc = Layout::union(&[&b_layout, &c_layout]);
		let left_union = Layout::union(&[&ab, &c_layout]);
		let right_union = Layout::union(&[&a_layout, &bc]);
		let frames = frames_for(&[&a_layout, &b_layout, &c_layout, &ab, &bc, &left_union, &right_union]);

		let a = || LeveledSourceNode {
			layout: a_layout.clone(),
			elements: vec![10., 11.],
			field: Some((Opacity::NAME, 0.5)),
		};
		let b = || LeveledSourceNode {
			layout: b_layout.clone(),
			elements: vec![100., 101., 102.],
			field: Some((Length::NAME, 7.)),
		};
		let c = || LeveledSourceNode {
			layout: c_layout.clone(),
			elements: vec![1000.],
			field: Some((Opacity::NAME, 0.25)),
		};
		let meta = || core_types::record::LayoutMeta {
			sources: vec![0, 1],
			reads: vec![],
			element: core_types::record::ElementSpec::Carried,
			writes: vec![],
			removes: vec![],
			level_delta: 0,
			folded: None,
		};

		let left_inner = install(
			ExtendNode::new(RecordSource::new(a(), &a_layout, &ab), RecordSource::new(b(), &b_layout, &ab), &ab),
			meta(),
			&[Some(&a_layout), Some(&b_layout)],
		);
		let left = install(
			ExtendNode::new(RecordSource::new(left_inner, &ab, &left_union), RecordSource::new(c(), &c_layout, &left_union), &left_union),
			meta(),
			&[Some(&ab), Some(&c_layout)],
		);
		let right_inner = install(
			ExtendNode::new(RecordSource::new(b(), &b_layout, &bc), RecordSource::new(c(), &c_layout, &bc), &bc),
			meta(),
			&[Some(&b_layout), Some(&c_layout)],
		);
		let right = install(
			ExtendNode::new(RecordSource::new(a(), &a_layout, &right_union), RecordSource::new(right_inner, &bc, &right_union), &right_union),
			meta(),
			&[Some(&a_layout), Some(&bc)],
		);

		assert_eq!(left.extent_at(&ctx, 0, &frames), GPoll::Final(Extent::Exactly(6)));
		assert_eq!(right.extent_at(&ctx, 0, &frames), GPoll::Final(Extent::Exactly(6)));

		let head = ctx.index_head();
		let check = |poll: GPoll<core_types::record::ServedRecord>, lane: usize, (element, opacity, length): (f64, f64, f64)| {
			let GPoll::Final(served) = poll else {
				panic!("expected a final record at lane {lane}");
			};
			assert_eq!(served.element::<f64>(), element, "lane {lane}");
			assert_eq!(served.attr::<Opacity>(), opacity, "lane {lane}");
			assert_eq!(served.attr::<Length>(), length, "lane {lane}");
		};
		let expected = [(10., 0.5, 0.), (11., 0.5, 0.), (100., 1., 7.), (101., 1., 7.), (102., 1., 7.), (1000., 0.25, 0.)];
		for (lane, &row) in expected.iter().enumerate() {
			let scoped = ctx.promoted(&head, lane as u64);
			check(core_types::record::capture(&left, &scoped, &frames), lane, row);
			check(core_types::record::capture(&right, &scoped, &frames), lane, row);
		}
	}

	#[test]
	fn reducer_folds_across_the_extend_seam() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let base_layout = leveled_f64_layout(&[Opacity::NAME]);
		let new_layout = leveled_f64_layout(&[Length::NAME]);
		let union = Layout::union(&[&base_layout, &new_layout]);
		let out = f64_layout(&[]);
		let frames = frames_for(&[&base_layout, &new_layout, &union, &out]);

		let base = LeveledSourceNode {
			layout: base_layout.clone(),
			elements: vec![10., 11.],
			field: Some((Opacity::NAME, 0.5)),
		};
		let new = LeveledSourceNode {
			layout: new_layout.clone(),
			elements: vec![100., 101., 102.],
			field: Some((Length::NAME, 7.)),
		};
		let meta = core_types::record::LayoutMeta {
			sources: vec![0, 1],
			reads: vec![],
			element: core_types::record::ElementSpec::Carried,
			writes: vec![],
			removes: vec![],
			level_delta: 0,
			folded: None,
		};
		let extend = install(
			ExtendNode::new(RecordSource::new(base, &base_layout, &union), RecordSource::new(new, &new_layout, &union), &union),
			meta,
			&[Some(&base_layout), Some(&new_layout)],
		);
		let wire = Node::<ContextImpl>::layout(&extend).clone();

		let head = ctx.index_head();
		let per_lane: f64 = (0..5u64)
			.map(|lane| {
				let GPoll::Final(served) = core_types::record::capture(&extend, &ctx.promoted(&head, lane), &frames) else {
					panic!("expected a final record at lane {lane}");
				};
				served.element::<f64>()
			})
			.sum();

		let node = install_flip(SumNode::new(extend, &wire), &out);
		let GPoll::Final(served) = core_types::record::capture(&node, &ctx, &frames) else {
			panic!("expected a final record");
		};
		// The fold's batch walks both sides of the seam in one range.
		let folded = served.element::<f64>();
		assert_eq!(folded, per_lane);
		assert_eq!(folded, 324.);
	}

	#[test]
	fn extend_forwards_matching_inner_extents_and_rejects_ragged() {
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let content_layout = leveled_f64_layout(&[]);
		let meta = || core_types::record::LayoutMeta {
			sources: vec![0],
			reads: vec![],
			element: core_types::record::ElementSpec::Carried,
			writes: vec![],
			removes: vec![],
			level_delta: 1,
			folded: None,
		};
		let extend_meta = || core_types::record::LayoutMeta {
			sources: vec![0, 1],
			reads: vec![],
			element: core_types::record::ElementSpec::Carried,
			writes: vec![],
			removes: vec![],
			level_delta: 0,
			folded: None,
		};
		let (base_count, base_count_layout) = lifted_value(2u32);
		let (base_reverse, base_reverse_layout) = lifted_value(false);
		let (new_count, new_count_layout) = lifted_value(4u32);
		let (new_reverse, new_reverse_layout) = lifted_value(false);
		let frames = frames_for(&[&content_layout, &base_count_layout, &new_count_layout]);

		let repeat = |elements: Vec<f64>, count_edge, reverse_edge, count_layout: &Layout, reverse_layout: &Layout| {
			let content = LeveledSourceNode {
				layout: content_layout.clone(),
				elements,
				field: None,
			};
			install(
				RepeatNode::new(
					RecordSource::new(content, &content_layout, &content_layout),
					count_edge,
					reverse_edge,
					&content_layout,
					count_layout,
					reverse_layout,
				),
				meta(),
				&[Some(&content_layout)],
			)
		};
		let base = repeat(vec![1., 2., 3.], base_count, base_reverse, &base_count_layout, &base_reverse_layout);
		let two_level = Node::<ContextImpl>::layout(&base).clone();
		assert_eq!(two_level.depth, 2);

		// Matching inner extents: the top concatenates, the inner forwards.
		let new = repeat(vec![10., 20., 30.], new_count, new_reverse, &new_count_layout, &new_reverse_layout);
		let node = install(ExtendNode::new(base, new, &two_level), extend_meta(), &[Some(&two_level), Some(&two_level)]);
		let out = Node::<ContextImpl>::layout(&node).clone();
		assert_eq!(out.depth, 2);
		assert_eq!(node.extent_at(&ctx, 1, &frames), GPoll::Final(Extent::Exactly(6)), "the top level sums both sides' copies");
		assert_eq!(node.extent_at(&ctx, 0, &frames), GPoll::Final(Extent::Exactly(3)), "the inner level forwards the shared extent");

		let head = ctx.index_head();
		for (lane, element) in [(0u64, 1.), (5, 3.), (6, 10.), (17, 30.)] {
			let GPoll::Final(served) = core_types::record::capture(&node, &ctx.promoted(&head, lane), &frames) else {
				panic!("expected a final record at lane {lane}");
			};
			assert_eq!(served.element::<f64>(), element, "flat lane {lane}");
		}

		// Mismatched inner extents: the top still sums, the inner query errors.
		let (base_count, base_count_layout) = lifted_value(2u32);
		let (base_reverse, base_reverse_layout) = lifted_value(false);
		let (new_count, new_count_layout) = lifted_value(4u32);
		let (new_reverse, new_reverse_layout) = lifted_value(false);
		let base = repeat(vec![1., 2., 3.], base_count, base_reverse, &base_count_layout, &base_reverse_layout);
		let new = repeat(vec![10., 20., 30., 40.], new_count, new_reverse, &new_count_layout, &new_reverse_layout);
		let ragged = install(ExtendNode::new(base, new, &two_level), extend_meta(), &[Some(&two_level), Some(&two_level)]);
		assert_eq!(ragged.extent_at(&ctx, 1, &frames), GPoll::Final(Extent::Exactly(6)));
		assert!(matches!(ragged.extent_at(&ctx, 0, &frames), GPoll::Error(_)), "ragged inner extents must not report a flat total");
	}

	#[test]
	fn omit_element_shrinks_the_level_and_shifts_the_tail() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = leveled_f64_layout(&[]);
		let frames = frames_for(&[&layout]);
		let meta = || core_types::record::LayoutMeta {
			sources: vec![0],
			reads: vec![],
			element: core_types::record::ElementSpec::Carried,
			writes: vec![],
			removes: vec![],
			level_delta: 0,
			folded: None,
		};
		let build = |index: f64| {
			let content = LeveledSourceNode {
				layout: layout.clone(),
				elements: vec![10., 11., 12.],
				field: None,
			};
			let (index_edge, index_layout) = lifted_value(index);
			install(
				OmitElementNode::new(RecordSource::new(content, &layout, &layout), index_edge, &layout, &index_layout),
				meta(),
				&[Some(&layout)],
			)
		};

		let head = ctx.index_head();
		for (index, expected) in [(1., vec![10., 12.]), (-1., vec![10., 11.]), (5., vec![10., 11., 12.])] {
			let node = build(index);
			assert_eq!(node.extent_at(&ctx, 0, &frames), GPoll::Final(Extent::Exactly(expected.len())), "omit at {index}");
			for (lane, &element) in expected.iter().enumerate() {
				let scoped = ctx.promoted(&head, lane as u64);
				let GPoll::Final(served) = core_types::record::capture(&node, &scoped, &frames) else {
					panic!("expected a final record");
				};
				assert_eq!(served.element::<f64>(), element, "omit at {index}, lane {lane}");
			}
		}
	}

	#[test]
	fn index_elements_selects_one_lane_with_its_attrs() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = leveled_f64_layout(&[Opacity::NAME]);
		let frames = frames_for(&[&layout]);
		let build = |index: f64| {
			let content = LeveledSourceNode {
				layout: layout.clone(),
				elements: vec![10., 11., 12.],
				field: Some((Opacity::NAME, 0.5)),
			};
			let (index_edge, index_layout) = lifted_value(index);
			let meta = core_types::record::LayoutMeta {
				sources: vec![0],
				reads: vec![],
				element: core_types::record::ElementSpec::Carried,
				writes: vec![],
				removes: vec![],
				level_delta: 0,
				folded: None,
			};
			install(
				IndexElementsNode::new(RecordSource::new(content, &layout, &layout), index_edge, &layout, &index_layout),
				meta,
				&[Some(&layout)],
			)
		};

		let head = ctx.index_head();
		for (index, expected) in [(1., Some(11.)), (-1., Some(12.)), (9., None)] {
			let node = build(index);
			assert_eq!(node.extent_at(&ctx, 0, &frames), GPoll::Final(Extent::Exactly(expected.is_some() as usize)), "index {index}");
			let Some(element) = expected else { continue };
			let scoped = ctx.promoted(&head, 0);
			let GPoll::Final(served) = core_types::record::capture(&node, &scoped, &frames) else {
				panic!("expected a final record");
			};
			assert_eq!(served.element::<f64>(), element);
			// The selected item keeps its attributes.
			assert_eq!(served.attr::<Opacity>(), 0.5);
		}
	}

	#[test]
	fn extract_element_reads_the_bare_element_or_the_default() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = leveled_f64_layout(&[]);
		let out = f64_layout(&[]);
		let frames = frames_for(&[&layout, &out]);

		for (index, expected) in [(1., 11.), (-1., 12.), (9., 0.)] {
			let content = LeveledSourceNode {
				layout: layout.clone(),
				elements: vec![10., 11., 12.],
				field: None,
			};
			let (index_edge, index_layout) = lifted_value(index);
			let node = install_flip(ExtractElementNode::new(RecordSource::new(content, &layout, &layout), index_edge, &layout, &index_layout), &out);
			let GPoll::Final(served) = core_types::record::capture(&node, &ctx, &frames) else {
				panic!("expected a final record");
			};
			assert_eq!(served.element::<f64>(), expected, "extract at {index}");
		}
	}

	#[test]
	fn mirror_reflects_about_the_levels_center() {
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = Layout::default().with_writes(1, core_types::record::element_write::<f64>(), &[core_types::record::FieldWrite::of::<Transform>(0)]);
		let frames = frames_for(&[&layout]);
		let content = |rows: &[(f64, f64)]| LeveledTransformSource {
			layout: layout.clone(),
			rows: rows.iter().map(|&(element, x)| (element, DAffine2::from_translation(glam::DVec2::new(x, 0.)))).collect(),
		};
		let rows = [(1., 10.), (2., 30.), (3., 20.)];
		let build = |keep: bool| {
			install(
				MirrorNode::new(RecordSource::new(content(&rows), &layout, &layout), ValueSource::new(keep)),
				mirror_layout_meta(),
				&[Some(&layout)],
			)
		};

		// Center of translations {10, 30, 20} is 20; reflection x' = 40 - x.
		let kept = build(true);
		let out = Node::<ContextImpl>::layout(&kept).clone();
		assert_eq!(out.depth, 1);
		assert_eq!(kept.extent_at(&ctx, 0, &frames), GPoll::Final(Extent::Exactly(6)));
		let expected = [(1., 10.), (2., 30.), (3., 20.), (1., 30.), (2., 10.), (3., 20.)];
		let head = ctx.index_head();
		for (lane, &(element, x)) in expected.iter().enumerate() {
			let scoped = ctx.promoted(&head, lane as u64);
			let GPoll::Final(served) = core_types::record::capture(&kept, &scoped, &frames) else {
				panic!("expected a final record");
			};
			assert_eq!(served.element::<f64>(), element, "lane {lane}");
			let transform: DAffine2 = served.attr::<Transform>();
			assert_eq!(transform.translation.x, x, "lane {lane}");
		}

		let replaced = build(false);
		assert_eq!(replaced.extent_at(&ctx, 0, &frames), GPoll::Final(Extent::Exactly(3)));
		let GPoll::Final(served) = core_types::record::capture(&replaced, &ctx.promoted(&head, 0), &frames) else {
			panic!("expected a final record");
		};
		let transform: DAffine2 = served.attr::<Transform>();
		assert_eq!(transform.translation.x, 30., "without originals every lane reflects");
	}

	#[test]
	fn a_gathered_lane_carries_its_whole_record() {
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		// The subject carries Transform, which the kernel never declares.
		let layout = Layout::default().with_writes(1, core_types::record::element_write::<f64>(), &[core_types::record::FieldWrite::of::<Transform>(0)]);
		let frames = frames_for(&[&layout]);
		let rows = [(1., 10.), (2., 30.), (3., 20.)];
		let content = LeveledTransformSource {
			layout: layout.clone(),
			rows: rows.iter().map(|&(element, x)| (element, DAffine2::from_translation(glam::DVec2::new(x, 0.)))).collect(),
		};
		let node = install(
			ReverseLanesNode::new(RecordSource::new(content, &layout, &layout), ValueSource::new(0.25)),
			reverse_lanes_layout_meta(),
			&[Some(&layout)],
		);

		let out = Node::<ContextImpl>::layout(&node).clone();
		assert_eq!(out.depth, 1, "gathering preserves the subject's depth");
		assert!(out.offset_of(<Transform as AttributeMarker>::NAME, 0).is_some(), "the undeclared attribute survives");
		assert_eq!(node.extent_at(&ctx, 0, &frames), GPoll::Final(Extent::Exactly(3)));

		let head = ctx.index_head();
		for (lane, &(element, x)) in rows.iter().rev().enumerate() {
			let GPoll::Final(served) = core_types::record::capture(&node, &ctx.promoted(&head, lane as u64), &frames) else {
				panic!("expected a final record");
			};
			assert_eq!(served.element::<f64>(), element, "lane {lane} takes the gathered element");
			let transform: DAffine2 = served.attr::<Transform>();
			assert_eq!(transform.translation.x, x, "lane {lane} carries the gathered transform");
			let opacity: f64 = served.attr::<Opacity>();
			assert_eq!(opacity, 0.25, "lane {lane} takes the declared write");
		}
	}

	#[test]
	fn batch_lanes_match_per_lane_eval() {
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = Layout::default().with_writes(1, core_types::record::element_write::<f64>(), &[core_types::record::FieldWrite::of::<Transform>(0)]);
		let frames = frames_for(&[&layout]);
		let content = LeveledTransformSource {
			layout: layout.clone(),
			rows: [(1., 10.), (2., 30.), (3., 20.)]
				.iter()
				.map(|&(element, x)| (element, DAffine2::from_translation(glam::DVec2::new(x, 0.))))
				.collect(),
		};
		let node = install(
			MirrorNode::new(RecordSource::new(content, &layout, &layout), ValueSource::new(true)),
			mirror_layout_meta(),
			&[Some(&layout)],
		);
		let out = Node::<ContextImpl>::layout(&node).clone();
		let head = ctx.index_head();
		let scoped = ctx.promoted(&head, 0);

		assert!(matches!(node.eval_batch(&scoped, 0..6, None, &frames), core_types::node::BatchStatus::NeedBuffer));
		let mut scratch = vec![std::mem::MaybeUninit::<u64>::uninit(); 6 * out.lane_stride() / 8];
		let core_types::node::BatchStatus::Filled(batch, finality, _) = node.eval_batch(&scoped, 0..6, Some(&mut scratch), &frames) else {
			panic!("expected a filled batch");
		};
		assert_eq!(finality, core_types::gpoll::Finality::AllFinal);
		let batch = batch.into_shared();
		assert_eq!(batch.len(), 6);
		for lane in 0..6 {
			let GPoll::Final(served) = core_types::record::capture(&node, &ctx.promoted(&head, lane as u64), &frames) else {
				panic!("expected a final record");
			};
			assert_eq!(unsafe { batch.get(lane).element::<f64>() }, served.element::<f64>(), "lane {lane}");
			let batched: DAffine2 = batch.get(lane).attr::<Transform>();
			let single: DAffine2 = served.attr::<Transform>();
			assert_eq!(batched, single, "lane {lane}");
		}
	}

	/// The compiler clears an input's bit when its source reads the addressed
	/// lane; the batch must then re-evaluate that source per lane, since hoisting
	/// a lane-varying value serves the range's first lane to all of them.
	#[test]
	fn batch_rebinds_an_eager_input_the_compiler_cannot_prove_invariant() {
		fn counting_value(value: bool, evals: &std::cell::Cell<u32>) -> LiftedSource<bool, impl for<'c> Fn(&ContextImpl<'c>) -> GPoll<bool> + '_> {
			LiftedSource::new(move |_: &ContextImpl<'_>| {
				evals.set(evals.get() + 1);
				GPoll::Final(value)
			})
		}

		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = Layout::default().with_writes(1, core_types::record::element_write::<f64>(), &[core_types::record::FieldWrite::of::<Transform>(0)]);
		let frames = frames_for(&[&layout]);
		let content = LeveledTransformSource {
			layout: layout.clone(),
			rows: [(1., 10.), (2., 30.), (3., 20.)]
				.iter()
				.map(|&(element, x)| (element, DAffine2::from_translation(glam::DVec2::new(x, 0.))))
				.collect(),
		};
		let evals = std::cell::Cell::new(0u32);
		let mut node = MirrorNode::new(RecordSource::new(content, &layout, &layout), counting_value(true, &evals));
		let resolved = core_types::record::RecordLayout {
			lane_invariant: 0,
			..mirror_layout_meta().resolve(&[Some(&layout)])
		};
		<_ as Node<ContextImpl<'static>>>::set_layout(&mut node, resolved);

		let out = Node::<ContextImpl>::layout(&node).clone();
		let head = ctx.index_head();
		let scoped = ctx.promoted(&head, 0);

		let mut scratch = vec![std::mem::MaybeUninit::<u64>::uninit(); 6 * out.lane_stride() / 8];
		let core_types::node::BatchStatus::Filled(batch, ..) = node.eval_batch(&scoped, 0..6, Some(&mut scratch), &frames) else {
			panic!("expected a filled batch");
		};
		assert_eq!(batch.len(), 6);
		assert_eq!(evals.get(), 6, "a lane-varying eager value binds once per lane");
	}

	#[test]
	fn batch_binds_eager_inputs_once() {
		fn counting_value(value: bool, evals: &std::cell::Cell<u32>) -> LiftedSource<bool, impl for<'c> Fn(&ContextImpl<'c>) -> GPoll<bool> + '_> {
			LiftedSource::new(move |_: &ContextImpl<'_>| {
				evals.set(evals.get() + 1);
				GPoll::Final(value)
			})
		}

		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = Layout::default().with_writes(1, core_types::record::element_write::<f64>(), &[core_types::record::FieldWrite::of::<Transform>(0)]);
		let frames = frames_for(&[&layout]);
		let content = LeveledTransformSource {
			layout: layout.clone(),
			rows: [(1., 10.), (2., 30.), (3., 20.)]
				.iter()
				.map(|&(element, x)| (element, DAffine2::from_translation(glam::DVec2::new(x, 0.))))
				.collect(),
		};
		let evals = std::cell::Cell::new(0u32);
		let node = install(
			MirrorNode::new(RecordSource::new(content, &layout, &layout), counting_value(true, &evals)),
			mirror_layout_meta(),
			&[Some(&layout)],
		);
		let out = Node::<ContextImpl>::layout(&node).clone();
		let head = ctx.index_head();
		let scoped = ctx.promoted(&head, 0);

		let mut scratch = vec![std::mem::MaybeUninit::<u64>::uninit(); 6 * out.lane_stride() / 8];
		let core_types::node::BatchStatus::Filled(batch, ..) = node.eval_batch(&scoped, 0..6, Some(&mut scratch), &frames) else {
			panic!("expected a filled batch");
		};
		assert_eq!(batch.len(), 6);
		assert_eq!(evals.get(), 1, "an eager value binds once per batch");
	}

	#[test]
	fn fold_collapses_a_deeper_wire() {
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let base = f64_layout(&[]);
		let leveled_content = repeat_opacity_layout(&base);
		let (count_edge, count_layout) = lifted_value(2u32);
		let (reverse_edge, reverse_layout) = lifted_value(false);
		let frames = frames_for(&[&base, &leveled_content, &count_layout, &reverse_layout]);

		// Element = the outer copy, so the total fold sums across both copies.
		let content = install(
			RepeatOpacityNode::new(IndexSourceNode { layout: base.clone() }, ValueSource::new(3u32), &base),
			repeat_opacity_layout_meta(),
			&[Some(&base)],
		);
		let meta = core_types::record::LayoutMeta {
			sources: vec![0],
			reads: vec![],
			element: core_types::record::ElementSpec::Carried,
			writes: vec![],
			removes: vec![],
			level_delta: 1,
			folded: None,
		};
		let nested = install(
			RepeatNode::new(
				RecordSource::new(content, &leveled_content, &leveled_content),
				count_edge,
				reverse_edge,
				&leveled_content,
				&count_layout,
				&reverse_layout,
			),
			meta,
			&[Some(&leveled_content)],
		);
		let two_level = Node::<ContextImpl>::layout(&nested).clone();
		assert_eq!(two_level.depth, 2);

		let node = install(SumNode::new(nested, &two_level), sum_layout_meta(), &[Some(&two_level)]);
		let out = Node::<ContextImpl>::layout(&node).clone();
		assert_eq!(out.depth, 0, "the fold consumes the whole wire");
		let GPoll::Final(served) = core_types::record::capture(&node, &ctx, &frames) else {
			panic!("expected a final record");
		};
		// Two copies of three lanes, each lane the copy index: 0 * 3 + 1 * 3.
		assert_eq!(served.element::<f64>(), 3.);
	}

	#[test]
	fn nested_fold_collapses_two_levels() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let base = f64_layout(&[]);
		let leveled_content = repeat_opacity_layout(&base);
		let (count_edge, count_layout) = lifted_value(2u32);
		let (reverse_edge, reverse_layout) = lifted_value(false);
		let out = f64_layout(&[]);
		let frames = frames_for(&[&base, &leveled_content, &count_layout, &reverse_layout, &out]);

		let content = install(
			RepeatOpacityNode::new(bare_source(&base, 7.), ValueSource::new(3u32), &base),
			repeat_opacity_layout_meta(),
			&[Some(&base)],
		);
		let meta = core_types::record::LayoutMeta {
			sources: vec![0],
			reads: vec![],
			element: core_types::record::ElementSpec::Carried,
			writes: vec![],
			removes: vec![],
			level_delta: 1,
			folded: None,
		};
		let nested = install(
			RepeatNode::new(
				RecordSource::new(content, &leveled_content, &leveled_content),
				count_edge,
				reverse_edge,
				&leveled_content,
				&count_layout,
				&reverse_layout,
			),
			meta,
			&[Some(&leveled_content)],
		);
		let two_level = Node::<ContextImpl>::layout(&nested).clone();
		assert_eq!(two_level.depth, 2);

		let node = install_flip(SumNestedNode::new(nested, &two_level), &out);
		let GPoll::Final(served) = core_types::record::capture(&node, &ctx, &frames) else {
			panic!("expected a final record");
		};
		// Two copies of three lanes of 7: the nested fold flattens 2 x 3.
		assert_eq!(served.element::<f64>(), 42.);
	}

	#[test]
	fn read_row_exposes_the_vararg_items_as_lanes() {
		let frames = core_types::record::test_frames(1 << 16);
		use core_types::Color;

		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let node = install(crate::context::ReadColorRowNode::new(), crate::context::read_color_row_layout_meta(), &[]);
		let out = Node::<ContextImpl>::layout(&node).clone();
		assert_eq!(out.depth, 1);
		// No row pushed: an empty level, matching the legacy empty-list return.
		assert_eq!(node.extent_at(&ctx, 0, &frames), GPoll::Final(Extent::Exactly(0)));

		let mut item = core_types::list::List::new_from_element(Color::BLACK);
		item.push(core_types::list::Item::new_from_element(Color::WHITE));
		let scoped = ctx.push_vararg(&item);
		let base = scoped.ctx();
		assert_eq!(node.extent_at(&base, 0, &frames), GPoll::Final(Extent::Exactly(2)));

		let head = base.index_head();
		for (lane, expected) in [(0u64, Color::BLACK), (1, Color::WHITE)] {
			let GPoll::Final(served) = core_types::record::capture(&node, &base.promoted(&head, lane), &frames) else {
				panic!("expected a final record at lane {lane}");
			};
			assert_eq!(served.element::<Color>(), expected, "lane {lane}");
		}
	}

	#[test]
	fn the_stamp_writes_the_layer_path_on_each_lane() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source_layout = leveled_f64_layout(&[]);
		let frames = frames_for(&[&source_layout]);
		let source = LeveledSourceNode {
			layout: source_layout.clone(),
			elements: vec![10., 11.],
			field: None,
		};
		let path = vec![NodeId(7), NodeId(8)];
		let node = install(
			StampLayerPathNode::new(RecordSource::new(source, &source_layout, &source_layout), ValueSource::new(path.clone()), &source_layout),
			stamp_layer_path_layout_meta(),
			&[Some(&source_layout)],
		);
		let out = Node::<ContextImpl>::layout(&node).clone();
		let offset = out.offset_of("editor:layer_path", 0).expect("the stamp writes the layer path");

		let head = ctx.index_head();
		for (lane, element) in [(0u64, 10.), (1, 11.)] {
			let lane_frames = frames.scope();
			let GPoll::Final(value) = core_types::record::serve_input(&node, &ctx.promoted(&head, lane), &lane_frames) else {
				panic!("expected a final record at lane {lane}");
			};
			let rec = out.rec(&value);
			assert_eq!(unsafe { rec.element::<f64>() }, element, "lane {lane}");
			assert_eq!(unsafe { rec.read::<&[NodeId]>(offset) }, path.as_slice(), "lane {lane}");
		}
	}

	#[test]
	fn repeat_decomposes_a_lower_bound_level() {
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let leveled = leveled_f64_layout(&[]);
		let out = f64_layout(&[]);
		let frames = frames_for(&[&leveled, &out]);
		let (count_edge, count_layout) = lifted_value(2u32);
		let (reverse_edge, reverse_layout) = lifted_value(false);
		let meta = core_types::record::LayoutMeta {
			sources: vec![0],
			reads: vec![],
			element: core_types::record::ElementSpec::Carried,
			writes: vec![],
			removes: vec![],
			level_delta: 1,
			folded: None,
		};
		let repeat = install(
			RepeatNode::new(
				RecordSource::new(DrainSourceNode { layout: leveled.clone(), count: 5 }, &leveled, &leveled),
				count_edge,
				reverse_edge,
				&leveled,
				&count_layout,
				&reverse_layout,
			),
			meta,
			&[Some(&leveled)],
		);
		let two_level = Node::<ContextImpl>::layout(&repeat).clone();
		assert_eq!(two_level.depth, 2);

		// The inner count comes from probing the lower-bound level, so the
		// flat index decomposes across both copies.
		let head = ctx.index_head();
		for (lane, expected) in [(0u64, 0.), (7, 2.), (9, 4.)] {
			let GPoll::Final(served) = core_types::record::capture(&repeat, &ctx.promoted(&head, lane), &frames) else {
				panic!("expected a final record at lane {lane}");
			};
			assert_eq!(served.element::<f64>(), expected, "lane {lane}");
		}

		// A full fold over the composite drains through the structure node;
		// the partial fold stays excluded with M3.
		let node = install_flip(SumNestedNode::new(repeat, &two_level), &out);
		let GPoll::Final(served) = core_types::record::capture(&node, &ctx, &frames) else {
			panic!("expected a final record");
		};
		assert_eq!(served.element::<f64>(), 20.);
	}

	#[test]
	fn a_leveled_value_source_serves_its_items_as_lanes() {
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source = core_types::value::LeveledValueSource::new(vec![1.5, 2.25, 3.75]);
		let leveled = Node::<ContextImpl>::layout(&source).clone();
		let out = f64_layout(&[]);
		let frames = frames_for(&[&leveled, &out]);

		let node = install_flip(SumNode::new(source, &leveled), &out);
		let GPoll::Final(served) = core_types::record::capture(&node, &ctx, &frames) else {
			panic!("expected a final record");
		};
		assert_eq!(served.element::<f64>(), 7.5);
	}

	#[test]
	fn reducer_drains_a_lower_bound_level() {
		let arena = Arena::new(1 << 16).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let leveled = leveled_f64_layout(&[]);
		let out = f64_layout(&[]);
		let frames = frames_for(&[&leveled, &out]);

		// 5 lanes end inside the first guess; 20 force a full first fill, a
		// hint-seeded regrow, and a short second fill.
		for count in [5usize, 20] {
			let source = DrainSourceNode { layout: leveled.clone(), count };
			let node = install_flip(SumNode::new(source, &leveled), &out);
			let GPoll::Final(served) = core_types::record::capture(&node, &ctx, &frames) else {
				panic!("expected a final record at count {count}");
			};
			let expected = (count * (count - 1) / 2) as f64;
			assert_eq!(served.element::<f64>(), expected, "count {count}");
		}
	}

	#[test]
	fn reducer_folds_a_repeated_level() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let base = f64_layout(&[]);
		let leveled = repeat_opacity_layout(&base);
		let out = f64_layout(&[]);
		let frames = frames_for(&[&base, &leveled, &out]);

		let repeat = install(
			RepeatOpacityNode::new(bare_source(&base, 7.), ValueSource::new(3u32), &base),
			repeat_opacity_layout_meta(),
			&[Some(&base)],
		);
		let node = install_flip(SumNode::new(repeat, &leveled), &out);
		assert_eq!(Node::<ContextImpl>::layout(&node).depth, 0, "the reducer collapsed the rank level");

		let GPoll::Final(served) = core_types::record::capture(&node, &ctx, &frames) else {
			panic!("expected a final record");
		};
		// sum(repeat(3, 7)) folds three copies of the element back to a scalar.
		assert_eq!(served.element::<f64>(), 21.);
	}

	#[test]
	fn reducer_folds_varying_copies_of_a_generic_repeat() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let base = f64_layout(&[]);
		let (count_edge, count_layout) = lifted_value(4u32);
		let (reverse_edge, reverse_layout) = lifted_value(false);
		let out = f64_layout(&[]);
		let frames = frames_for(&[&base, &count_layout, &out]);

		let meta = core_types::record::LayoutMeta {
			sources: vec![0],
			reads: vec![],
			element: core_types::record::ElementSpec::Carried,
			writes: vec![],
			removes: vec![],
			level_delta: 1,
			folded: None,
		};
		let repeat = install(
			RepeatNode::new(
				RecordSource::new(IndexSourceNode { layout: base.clone() }, &base, &base),
				count_edge,
				reverse_edge,
				&base,
				&count_layout,
				&reverse_layout,
			),
			meta,
			&[Some(&base)],
		);
		let leveled = Node::<ContextImpl>::layout(&repeat).clone();
		let node = install_flip(SumNode::new(repeat, &leveled), &out);

		let GPoll::Final(served) = core_types::record::capture(&node, &ctx, &frames) else {
			panic!("expected a final record");
		};
		// Every copy evaluates at its own index, so the lanes must be distinct
		// storage: sum(0 + 1 + 2 + 3), not four aliases of the last copy.
		assert_eq!(served.element::<f64>(), 6.);
	}

	#[test]
	fn layout_meta_folds_to_construction() {
		let base = f64_layout(&[]);
		let carried = multiply_opacity_layout(&base);

		assert_eq!(multiply_opacity_layout_meta().fold(&[Some(&base)]), multiply_opacity_layout(&base));
		assert_eq!(multiply_opacity_layout_meta().fold(&[Some(&carried)]), multiply_opacity_layout(&carried));
		assert_eq!(measure_layout_meta().fold(&[Some(&base)]), measure_layout(&base));
		assert_eq!(strip_opacity_layout_meta().fold(&[Some(&carried)]), strip_opacity_layout(&carried));
		assert_eq!(relength_layout_meta().fold(&[Some(&base)]), relength_layout(&base));
		assert_eq!(transfer_opacity_layout_meta().fold(&[Some(&base), None]), transfer_opacity_layout(&base));
		assert_eq!(source_opacity_layout_meta().fold(&[]), source_opacity_layout());
	}

	#[test]
	fn defaults_then_modify_then_stack() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source_layout = f64_layout(&[]);
		let modified = multiply_opacity_layout(&source_layout);
		let stacked = multiply_opacity_layout(&modified);
		let frames = frames_for(&[&source_layout, &modified, &stacked]);

		let chain = install(
			MultiplyOpacityNode::new(
				install(
					MultiplyOpacityNode::new(bare_source(&source_layout, 2.), ValueSource::new(0.5), &source_layout),
					multiply_opacity_layout_meta(),
					&[Some(&source_layout)],
				),
				ValueSource::new(0.5),
				&modified,
			),
			multiply_opacity_layout_meta(),
			&[Some(&modified)],
		);
		assert_eq!(Node::<ContextImpl>::layout(&chain), &stacked);
		let GPoll::Final(served) = core_types::record::capture(&chain, &ctx, &frames) else {
			panic!("expected a final record");
		};
		assert_eq!(served.element::<f64>(), 2.);
		assert_eq!(served.attr::<Opacity>(), 0.25);
	}

	#[test]
	fn tuple_write_element_and_attribute() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source_layout = f64_layout(&[]);
		let measured = measure_layout(&source_layout);
		let frames = frames_for(&[&source_layout, &measured]);

		let chain = install(MeasureNode::new(bare_source(&source_layout, -2.), &source_layout), measure_layout_meta(), &[Some(&source_layout)]);
		let GPoll::Final(served) = core_types::record::capture(&chain, &ctx, &frames) else {
			panic!("expected a final record");
		};
		assert_eq!(served.element::<f64>(), -2.);
		assert_eq!(served.attr::<Length>(), 2.);
	}

	#[test]
	fn elementwise_write_carries_unrelated_fields() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source_layout = f64_layout(&[]);
		let modified = multiply_opacity_layout(&source_layout);
		let measured = measure_layout(&modified);
		let frames = frames_for(&[&source_layout, &modified, &measured]);

		let chain = install(
			MeasureNode::new(
				install(
					MultiplyOpacityNode::new(bare_source(&source_layout, -2.), ValueSource::new(0.5), &source_layout),
					multiply_opacity_layout_meta(),
					&[Some(&source_layout)],
				),
				&modified,
			),
			measure_layout_meta(),
			&[Some(&modified)],
		);
		let GPoll::Final(served) = core_types::record::capture(&chain, &ctx, &frames) else {
			panic!("expected a final record");
		};
		assert_eq!(served.attr::<Opacity>(), 0.5);
		assert_eq!(served.attr::<Length>(), 2.);
	}

	#[test]
	fn reads_only_kernel_reads_the_declared_default_and_carries() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source_layout = f64_layout(&[]);
		let modified = multiply_opacity_layout(&source_layout);
		let shaded = shade_layout(&modified);
		let frames = frames_for(&[&source_layout, &modified, &shaded]);

		let bare = install(ShadeNode::new(bare_source(&source_layout, 4.), &source_layout), shade_layout_meta(), &[Some(&source_layout)]);
		let GPoll::Final(served) = core_types::record::capture(&bare, &ctx, &frames) else {
			panic!("expected a final record");
		};
		assert_eq!(served.element::<f64>(), 4.);

		let chain = install(
			ShadeNode::new(
				install(
					MultiplyOpacityNode::new(bare_source(&source_layout, 4.), ValueSource::new(0.5), &source_layout),
					multiply_opacity_layout_meta(),
					&[Some(&source_layout)],
				),
				&modified,
			),
			shade_layout_meta(),
			&[Some(&modified)],
		);
		let GPoll::Final(served) = core_types::record::capture(&chain, &ctx, &frames) else {
			panic!("expected a final record");
		};
		assert_eq!(served.element::<f64>(), 2.);
		assert_eq!(served.attr::<Opacity>(), 0.5);
	}

	#[test]
	fn token_passthrough_carries_any_element_type() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let f64_source = f64_layout(&[]);
		let f64_faded = fade_layout(&f64_source);
		let u32_source = Layout::default().with_writes(0, core_types::record::element_write::<u32>(), &[]);
		let u32_faded = fade_layout(&u32_source);
		let frames = frames_for(&[&f64_source, &f64_faded, &u32_source, &u32_faded]);

		let wide = install(
			FadeNode::new(bare_source(&f64_source, 8.), ValueSource::new(0.5), &f64_source),
			fade_layout_meta(),
			&[Some(&f64_source)],
		);
		let GPoll::Final(served) = core_types::record::capture(&wide, &ctx, &frames) else {
			panic!("expected a final record");
		};
		assert_eq!(served.element::<f64>(), 8.);
		assert_eq!(served.attr::<Opacity>(), 0.5);

		let narrow = install(
			FadeNode::new(
				RecordSourceNode {
					layout: u32_source.clone(),
					element: 7u32,
					fields: vec![],
					partial: false,
				},
				ValueSource::new(0.25),
				&u32_source,
			),
			fade_layout_meta(),
			&[Some(&u32_source)],
		);
		let GPoll::Final(served) = core_types::record::capture(&narrow, &ctx, &frames) else {
			panic!("expected a final record");
		};
		assert_eq!(served.element::<u32>(), 7);
		assert_eq!(served.attr::<Opacity>(), 0.25);
	}

	#[test]
	fn no_carrier_form_writes_a_fresh_record() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = source_opacity_layout();
		let frames = frames_for(&[&layout]);

		let node = install(
			SourceOpacityNode::new(ValueSource::new(()), ValueSource::new(3.), ValueSource::new(0.25)),
			source_opacity_layout_meta(),
			&[],
		);
		assert_eq!(Node::<ContextImpl>::layout(&node), &layout);
		let GPoll::Final(served) = core_types::record::capture(&node, &ctx, &frames) else {
			panic!("expected a final record");
		};
		assert_eq!(served.element::<f64>(), 3.);
		assert_eq!(served.attr::<Opacity>(), 0.25);
	}

	#[test]
	fn partial_carrier_downgrades_the_output() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source_layout = f64_layout(&[]);
		let modified = multiply_opacity_layout(&source_layout);
		let frames = frames_for(&[&source_layout, &modified]);

		let chain = install(
			MultiplyOpacityNode::new(
				RecordSourceNode {
					layout: source_layout.clone(),
					element: 1.,
					fields: vec![],
					partial: true,
				},
				ValueSource::new(0.5),
				&source_layout,
			),
			multiply_opacity_layout_meta(),
			&[Some(&source_layout)],
		);
		let GPoll::Partial(served) = core_types::record::capture(&chain, &ctx, &frames) else {
			panic!("expected a partial record");
		};
		assert_eq!(served.attr::<Opacity>(), 0.5);
	}

	#[test]
	fn interrupt_kernel_errors_stop_the_eval() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source_layout = f64_layout(&[]);
		let modified = checked_multiply_opacity_layout(&source_layout);
		let frames = frames_for(&[&source_layout, &modified]);

		let ok = install(
			CheckedMultiplyOpacityNode::new(bare_source(&source_layout, 1.), ValueSource::new(0.5), &source_layout),
			checked_multiply_opacity_layout_meta(),
			&[Some(&source_layout)],
		);
		let GPoll::Final(served) = core_types::record::capture(&ok, &ctx, &frames) else {
			panic!("expected a final record");
		};
		assert_eq!(served.attr::<Opacity>(), 0.5);

		let failing = install(
			CheckedMultiplyOpacityNode::new(bare_source(&source_layout, 1.), ValueSource::new(-1.), &source_layout),
			checked_multiply_opacity_layout_meta(),
			&[Some(&source_layout)],
		);
		let GPoll::Error(error) = core_types::record::serve_input(&failing, &ctx, &frames) else {
			panic!("expected an error");
		};
		assert!(error.kind == "negative factor");
	}

	#[test]
	fn lend_value_params_wire_into_record_kernels() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source_layout = f64_layout(&[]);
		let modified = multiply_opacity_layout(&source_layout);
		let scaled = scale_layout(&modified);
		let frames = frames_for(&[&source_layout, &modified, &scaled]);

		let chain = install(
			ScaleNode::new(
				install(
					MultiplyOpacityNode::new(bare_source(&source_layout, 2.), ValueSource::new(0.5), &source_layout),
					multiply_opacity_layout_meta(),
					&[Some(&source_layout)],
				),
				ValueSource::new(3.),
				&modified,
			),
			scale_layout_meta(),
			&[Some(&modified)],
		);
		let GPoll::Final(served) = core_types::record::capture(&chain, &ctx, &frames) else {
			panic!("expected a final record");
		};
		assert_eq!(served.element::<f64>(), 6.);
		assert_eq!(served.attr::<Opacity>(), 0.5);
	}

	#[test]
	fn secondary_input_reads_bind_to_their_own_wire() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let carrier_layout = f64_layout(&["opacity"]);
		let secondary_layout = f64_layout(&["opacity"]);
		let transferred = transfer_opacity_layout(&carrier_layout);
		let frames = frames_for(&[&carrier_layout, &secondary_layout, &transferred]);

		let chain = install(
			TransferOpacityNode::new(
				f64_record_source(&carrier_layout, 2., vec![("opacity", 0.5)]),
				f64_record_source(&secondary_layout, 3., vec![("opacity", 0.25)]),
				&carrier_layout,
				&secondary_layout,
			),
			transfer_opacity_layout_meta(),
			&[Some(&carrier_layout), None],
		);
		let GPoll::Final(served) = core_types::record::capture(&chain, &ctx, &frames) else {
			panic!("expected a final record");
		};
		assert_eq!(served.element::<f64>(), 5.);
		assert_eq!(served.attr::<Opacity>(), 0.125);

		let bare_secondary = f64_layout(&[]);
		let defaulted = install(
			TransferOpacityNode::new(
				f64_record_source(&carrier_layout, 2., vec![("opacity", 0.5)]),
				bare_source(&bare_secondary, 3.),
				&carrier_layout,
				&bare_secondary,
			),
			transfer_opacity_layout_meta(),
			&[Some(&carrier_layout), None],
		);
		let GPoll::Final(served) = core_types::record::capture(&defaulted, &ctx, &frames) else {
			panic!("expected a final record");
		};
		assert_eq!(served.attr::<Opacity>(), 0.5, "an absent secondary attribute reads its default");
	}

	#[test]
	fn a_flipped_node_carries_its_primary_inputs_fields() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source_layout = f64_layout(&["opacity"]);
		let factor = ValueSource::new(3.);
		let factor_layout = Node::<ContextImpl>::layout(&factor).clone();
		let frames = frames_for(&[&source_layout]);

		let node = install(
			BoostNode::new(f64_record_source(&source_layout, 2., vec![("opacity", 0.25)]), factor, &source_layout, &factor_layout),
			boost_layout_meta(),
			&[Some(&source_layout)],
		);
		let out_layout = Node::<ContextImpl>::layout(&node).clone();
		out_layout.offset_of(Opacity::NAME, 0).expect("the primary input's fields pass through to the output");
		let GPoll::Final(served) = core_types::record::capture(&node, &ctx, &frames) else {
			panic!("expected a final record");
		};
		assert_eq!(served.element::<f64>(), 6.);
		assert_eq!(served.attr::<Opacity>(), 0.25);
	}

	#[test]
	fn a_poll_kernel_carries_its_primary_inputs_fields() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source_layout = f64_layout(&["opacity"]);
		let (factor, factor_layout) = lifted_value(3.);
		let frames = frames_for(&[&source_layout]);

		let node = install(
			BoostPollNode::new(f64_record_source(&source_layout, 2., vec![("opacity", 0.25)]), factor, &source_layout, &factor_layout),
			boost_poll_layout_meta(),
			&[Some(&source_layout)],
		);
		let out_layout = Node::<ContextImpl>::layout(&node).clone();
		out_layout.offset_of(Opacity::NAME, 0).expect("the primary input's fields pass through the poll kernel");
		let GPoll::Final(served) = core_types::record::capture(&node, &ctx, &frames) else {
			panic!("expected a final record");
		};
		assert_eq!(served.element::<f64>(), 6.);
		assert_eq!(served.attr::<Opacity>(), 0.25);
	}

	#[test]
	fn a_byte_carried_spilled_borrow_parks_and_survives_the_carrier_eval() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let carrier_layout = f64_layout(&["opacity"]);
		let by_layout = f64_layout(&["opacity", "length"]);
		assert!(by_layout.frame_bytes() != 0, "the borrow must point into a spilled frame to exercise the park");
		let frames = frames_for(&[&carrier_layout, &by_layout]);

		let node = install(
			OffsetNode::new(
				f64_record_source(&carrier_layout, 2., vec![("opacity", 0.25)]),
				f64_record_source(&by_layout, 40., vec![]),
				&carrier_layout,
				&by_layout,
			),
			offset_layout_meta(),
			&[Some(&carrier_layout)],
		);
		let GPoll::Final(served) = core_types::record::capture(&node, &ctx, &frames) else {
			panic!("expected a final record");
		};
		assert_eq!(served.element::<f64>(), 42., "the parked borrow survives the carrier evaluation reusing its frame");
		assert_eq!(served.attr::<Opacity>(), 0.25);
	}

	struct InlineRuntime;

	impl core_types::runtime::Runtime for InlineRuntime {
		fn spawn(&self, _source: SourceId, mut future: core_types::runtime::SourceFuture) -> bool {
			let mut task_ctx = std::task::Context::from_waker(std::task::Waker::noop());
			assert!(future.as_mut().poll(&mut task_ctx).is_ready(), "the inline runtime completes tasks at spawn");
			true
		}
	}

	#[test]
	fn an_async_source_carries_its_primary_inputs_fields_around_the_slot() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source_layout = f64_layout(&["opacity"]);
		let (runtime, runtime_layout) = lifted_value(core_types::runtime::RuntimeHandle(std::sync::Arc::new(InlineRuntime)));
		let (source_id, source_id_layout) = lifted_value(7 as SourceId);
		let frames = frames_for(&[&source_layout]);

		let node = install(
			DoubleAsyncNode::new(
				f64_record_source(&source_layout, 3., vec![("opacity", 0.25)]),
				runtime,
				source_id,
				&source_layout,
				&runtime_layout,
				&source_id_layout,
			),
			double_async_layout_meta(),
			&[Some(&source_layout)],
		);
		let out_layout = Node::<ContextImpl>::layout(&node).clone();
		out_layout.offset_of(Opacity::NAME, 0).expect("the carrier's fields pass through the async source");

		let GPoll::Final(served) = core_types::record::capture(&node, &ctx, &frames) else {
			panic!("an inline completion is final on the spawning eval");
		};
		assert_eq!(served.element::<f64>(), 6.);
		assert_eq!(served.attr::<Opacity>(), 0.25);

		let GPoll::Final(served) = core_types::record::capture(&node, &ctx, &frames) else {
			panic!("a slot hit is final");
		};
		assert_eq!(served.element::<f64>(), 6., "the slot hit replays the element");
		assert_eq!(served.attr::<Opacity>(), 0.25, "the fields re-carry on every eval");
	}

	#[test]
	fn a_writing_async_source_lifts_its_slot_tuple_through_the_claim() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source_layout = f64_layout(&["length"]);
		let (runtime, _) = lifted_value(core_types::runtime::RuntimeHandle(std::sync::Arc::new(InlineRuntime)));
		let (source_id, _) = lifted_value(7 as SourceId);
		let frames = frames_for(&[&source_layout]);

		let node = install(
			FadeAsyncNode::new(
				f64_record_source(&source_layout, 3., vec![("length", 9.)]),
				ValueSource::new(0.5),
				runtime,
				source_id,
				&source_layout,
			),
			fade_async_layout_meta(),
			&[Some(&source_layout)],
		);
		assert_eq!(Node::<ContextImpl>::layout(&node), &fade_async_layout(&source_layout));

		let GPoll::Final(served) = core_types::record::capture(&node, &ctx, &frames) else {
			panic!("an inline completion is final on the spawning eval");
		};
		assert_eq!(served.element::<f64>(), 6.);
		assert_eq!(served.attr::<Opacity>(), 0.5, "the write lands through the claim, not the future");
		assert_eq!(served.attr::<Length>(), 9., "the carrier's other fields still pass through");

		let GPoll::Final(served) = core_types::record::capture(&node, &ctx, &frames) else {
			panic!("a slot hit is final");
		};
		assert_eq!(served.element::<f64>(), 6., "the slot hit replays the element");
		assert_eq!(served.attr::<Opacity>(), 0.5, "the slot hit replays the write");
		assert_eq!(served.attr::<Length>(), 9.);
	}

	#[test]
	fn a_writing_async_source_without_a_carrier_writes_a_fresh_record() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let (runtime, _) = lifted_value(core_types::runtime::RuntimeHandle(std::sync::Arc::new(InlineRuntime)));
		let (source_id, _) = lifted_value(7 as SourceId);
		let layout = measure_async_layout();
		let frames = frames_for(&[&layout]);

		let node = install(
			MeasureAsyncNode::new(ValueSource::new(()), ValueSource::new(-4.), runtime, source_id),
			measure_async_layout_meta(),
			&[],
		);
		assert_eq!(Node::<ContextImpl>::layout(&node), &layout);

		let GPoll::Final(served) = core_types::record::capture(&node, &ctx, &frames) else {
			panic!("an inline completion is final on the spawning eval");
		};
		assert_eq!(served.element::<f64>(), -4.);
		assert_eq!(served.attr::<Length>(), 4.);
	}

	#[test]
	fn an_owned_crossing_parks_its_reference_on_every_lift() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let (runtime, _) = lifted_value(core_types::runtime::RuntimeHandle(std::sync::Arc::new(InlineRuntime)));
		let (source_id, _) = lifted_value(7 as SourceId);
		let layout = tag_async_layout();
		let frames = frames_for(&[&layout]);

		let node = install(
			TagAsyncNode::new(ValueSource::new(()), ValueSource::new(3.), ValueSource::new("tagged".to_string()), runtime, source_id),
			tag_async_layout_meta(),
			&[],
		);
		assert_eq!(Node::<ContextImpl>::layout(&node), &layout);
		let offset = layout.offset_of("label", 0).expect("the source writes the label");

		for pass in ["the spawning eval", "the slot hit"] {
			let scoped = frames.scope();
			let GPoll::Final(value) = core_types::record::serve_input(&node, &ctx, &scoped) else {
				panic!("an inline completion is final on {pass}");
			};
			let rec = layout.rec(&value);
			assert_eq!(unsafe { rec.element::<f64>() }, 3., "{pass}");
			assert_eq!(unsafe { rec.read::<&str>(offset) }, "tagged", "{pass}");
		}
	}

	#[test]
	fn lazy_reads_bind_to_their_edge_and_leave_the_untaken_branch_unevaluated() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let unit = ValueSource::new(());
		let unit_layout = Node::<ContextImpl>::layout(&unit).clone();
		let content_layout = f64_layout(&["opacity"]);
		let frames = frames_for(&[&content_layout]);

		let run = |opacity: Option<f64>| {
			let evals = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
			let alternate = counting_source(evals.clone());
			let alternate_layout = Node::<ContextImpl>::layout(&alternate).clone();
			let (content_layout, fields) = match opacity {
				Some(value) => (content_layout.clone(), vec![("opacity", value)]),
				None => (f64_layout(&[]), vec![]),
			};
			let node = install_flip(
				FallbackNode::new(
					ValueSource::new(()),
					f64_record_source(&content_layout, 7., fields),
					alternate,
					&unit_layout,
					&content_layout,
					&alternate_layout,
				),
				&f64_layout(&[]),
			);
			let GPoll::Final(value) = core_types::record::serve_input(&node, &ctx, &frames) else {
				panic!("expected a final record");
			};
			let element = unsafe { Node::<ContextImpl>::layout(&node).rec(&value).element::<f64>() };
			(element, evals.load(std::sync::atomic::Ordering::Relaxed))
		};

		assert_eq!(run(Some(0.5)), (7., 0), "a visible content skips the alternate branch entirely");
		assert_eq!(run(Some(0.)), (21., 1), "a transparent content evaluates the alternate branch");
		assert_eq!(run(None), (7., 0), "an absent attribute reads its declared default");
	}

	#[test]
	fn remove_attr_leaves_the_layout_and_downstream_reads_the_default() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source_layout = f64_layout(&[]);
		let modified = multiply_opacity_layout(&source_layout);
		let stripped = strip_opacity_layout(&modified);
		assert!(stripped.offset_of(Opacity::NAME, 0).is_none(), "the removed name leaves the output layout");
		let shaded = shade_layout(&stripped);
		let frames = frames_for(&[&source_layout, &modified, &stripped, &shaded]);

		let chain = install(
			ShadeNode::new(
				install(
					StripOpacityNode::new(
						install(
							MultiplyOpacityNode::new(bare_source(&source_layout, 4.), ValueSource::new(0.5), &source_layout),
							multiply_opacity_layout_meta(),
							&[Some(&source_layout)],
						),
						&modified,
					),
					strip_opacity_layout_meta(),
					&[Some(&modified)],
				),
				&stripped,
			),
			shade_layout_meta(),
			&[Some(&stripped)],
		);
		let GPoll::Final(served) = core_types::record::capture(&chain, &ctx, &frames) else {
			panic!("expected a final record");
		};
		assert_eq!(served.element::<f64>(), 4., "a read after the removal yields the declared default");
	}

	#[test]
	fn mixed_writes_and_removes_destructure_in_tuple_order() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source_layout = f64_layout(&["opacity", "length"]);
		let relengthed = relength_layout(&source_layout);
		assert!(relengthed.offset_of(Opacity::NAME, 0).is_none());
		let frames = frames_for(&[&source_layout, &relengthed]);

		let chain = install(
			RelengthNode::new(f64_record_source(&source_layout, 3., vec![("opacity", 0.25), ("length", 9.)]), &source_layout),
			relength_layout_meta(),
			&[Some(&source_layout)],
		);
		let GPoll::Final(served) = core_types::record::capture(&chain, &ctx, &frames) else {
			panic!("expected a final record");
		};
		assert_eq!(served.element::<f64>(), 3.);
		assert_eq!(served.attr::<Length>(), 6.);
	}

	#[test]
	fn parked_reference_attributes_write_and_carry() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source_layout = f64_layout(&[]);
		let labeled = label_layout(&source_layout);
		let relabeled = label_layout(&labeled);
		let frames = frames_for(&[&source_layout, &labeled, &relabeled]);

		let chain = install(
			LabelNode::new(
				install(
					LabelNode::new(bare_source(&source_layout, 1.), ValueSource::new(String::from("a")), &source_layout),
					label_layout_meta(),
					&[Some(&source_layout)],
				),
				ValueSource::new(String::from("b")),
				&labeled,
			),
			label_layout_meta(),
			&[Some(&labeled)],
		);
		let GPoll::Final(value) = core_types::record::serve_input(&chain, &ctx, &frames) else {
			panic!("expected a final record");
		};
		let rec = relabeled.rec(&value);
		assert_eq!(unsafe { rec.element::<f64>() }, 1.);
		assert_eq!(unsafe { rec.read::<&str>(relabeled.offset_of(Label::NAME, 0).unwrap()) }, "ab");
	}

	#[test]
	fn census_fills_reference_defaults_from_static_data() {
		let source = f64_layout(&[]);
		let labeled = Layout::default().with_writes(0, core_types::record::element_write::<f64>(), &[core_types::record::FieldWrite::of::<Label>(0)]);

		let plan = core_types::record::SourcePlan::new(&source, &labeled).unwrap();
		let record = [5f64];
		let mut buffer = vec![0u64; labeled.size.div_ceil(8)];
		let translated = unsafe { plan.translate(Rec::new(record.as_ptr().cast()), buffer.as_mut_ptr().cast()) };
		assert_eq!(unsafe { translated.element::<f64>() }, 5.);
		assert_eq!(unsafe { translated.read::<&str>(labeled.offset_of(Label::NAME, 0).unwrap()) }, "");
	}

	fn f64_record_source(layout: &Layout, element: f64, fields: Vec<(&'static str, f64)>) -> RecordSourceNode<f64> {
		RecordSourceNode {
			layout: layout.clone(),
			element,
			fields,
			partial: false,
		}
	}

	#[test]
	fn record_monitor_forwards_and_recreates_from_its_snapshot() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];

		let layout = f64_layout(&["opacity"]);
		let frames = frames_for(&[&layout, &layout]);

		let monitor = crate::memo::MonitorNode::new(f64_record_source(&layout, 4., vec![("opacity", 0.25)]), &layout);
		let scope = scope_fixture(&generations, &arena);
		{
			let ctx = ContextImpl::root(&scope);
			let GPoll::Final(served) = core_types::record::capture(&monitor, &ctx, &frames) else {
				panic!("expected a final record");
			};
			assert_eq!(served.element::<f64>(), 4.);
		}

		let io = Node::<ContextImpl>::serialize(&monitor).unwrap();
		let snapshot = io.downcast_ref::<core_types::context::CtxSnapshot>().expect("the monitor serializes its context snapshot");
		let ctx = snapshot.rehydrate(&scope).expect("the arena holds the chains");
		let GPoll::Final(served) = core_types::record::capture(&monitor, &ctx, &frames) else {
			panic!("expected a final record");
		};
		assert_eq!(served.element::<f64>(), 4.);
		assert_eq!(served.field::<f64>("opacity", 0), 0.25);
	}

	#[test]
	fn routing_unions_branch_layouts_and_fills_census_defaults() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout_a = f64_layout(&["opacity"]);
		let layout_b = f64_layout(&["length"]);
		let union = Layout::union(&[&layout_a, &layout_b]);
		let frames = frames_for(&[&layout_a, &layout_b, &union, &union]);

		let taken = |second: bool| {
			let (condition, condition_layout) = lifted_value(second);
			PickNode::new(
				condition,
				RecordSource::new(f64_record_source(&layout_a, 1., vec![("opacity", 0.5)]), &layout_a, &union),
				RecordSource::new(f64_record_source(&layout_b, 3., vec![("length", 3.)]), &layout_b, &union),
				&union,
				&condition_layout,
			)
		};

		let GPoll::Final(served) = core_types::record::capture(&taken(false), &ctx, &frames) else {
			panic!("expected a final record");
		};
		assert_eq!(served.element::<f64>(), 1.);
		assert_eq!(served.field::<f64>("opacity", 0), 0.5);
		assert_eq!(served.field::<f64>("length", 0), 0.);

		let GPoll::Final(served) = core_types::record::capture(&taken(true), &ctx, &frames) else {
			panic!("expected a final record");
		};
		assert_eq!(served.element::<f64>(), 3.);
		assert_eq!(served.field::<f64>("opacity", 0), 1.);
		assert_eq!(served.field::<f64>("length", 0), 3.);
	}

	#[test]
	fn routing_provenance_survives_later_evaluations() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout_a = f64_layout(&["opacity"]);
		let layout_b = f64_layout(&["length"]);
		let union = Layout::union(&[&layout_a, &layout_b]);
		let frames = frames_for(&[&layout_a, &layout_b, &union, &union]);

		let (condition, condition_layout) = lifted_value(false);
		let chain = HoldFirstNode::new(
			condition,
			RecordSource::new(f64_record_source(&layout_a, 1., vec![("opacity", 0.5)]), &layout_a, &union),
			RecordSource::new(f64_record_source(&layout_b, 3., vec![("length", 3.)]), &layout_b, &union),
			&union,
			&condition_layout,
		);

		let GPoll::Final(served) = core_types::record::capture(&chain, &ctx, &frames) else {
			panic!("expected a final record");
		};
		assert_eq!(served.element::<f64>(), 1.);
		assert_eq!(served.field::<f64>("opacity", 0), 0.5);
		assert_eq!(served.field::<f64>("length", 0), 0.);
	}

	struct RealTimeProbe {
		layout: Layout,
	}

	impl<C: ExtractIndex + core_types::ExtractRealTime> Node<C> for RealTimeProbe {
		fn serve<'e, 'l>(&self, input: &C, slot: FrameClaim<'e, 'l>) -> GPoll<Served<'e>>
		where
			C: ExtractArena<ArenaRef = &'e Arena>,
		{
			let element: f64 = match core_types::context::ExtractRealTime::try_real_time(input) {
				Some(_) => 1.,
				None => 0.,
			};
			let mut frame = slot;
			let arena = ExtractArena::arena(input);
			if frame.element(element, arena).is_none() {
				return GPoll::arena_exhausted();
			}
			// SAFETY: the writes above complete the record of this layout.
			GPoll::Final(unsafe { frame.finish_served() })
		}

		fn layout(&self) -> &Layout {
			&self.layout
		}
	}

	#[test]
	fn context_modification_nullifies_for_the_inner_record_edge() {
		use core_types::context::{ContextFeatures, ContextModification};

		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = f64_layout(&[]);
		let frames = frames_for(&[&layout]);

		let probed = |features: ContextFeatures| {
			let (modification, modification_layout) = lifted_value(ContextModification::from_sources(features, &[]));
			let node = crate::context_modification::ContextModificationNode::new(RealTimeProbe { layout: layout.clone() }, modification, &layout, &modification_layout);
			assert_eq!(Node::<ContextImpl>::layout(&node), &layout);
			let GPoll::Final(served) = core_types::record::capture(&node, &ctx, &frames) else {
				panic!("expected a final record");
			};
			served.element::<f64>()
		};

		assert_eq!(probed(ContextFeatures::all()), 1., "kept features stay readable under the modification");
		assert_eq!(probed(ContextFeatures::empty()), 0., "nullified features read as absent for the inner edge");
	}

	#[test]
	fn context_modification_forwards_record_partiality() {
		use core_types::context::{ContextFeatures, ContextModification};

		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = f64_layout(&["opacity"]);
		let frames = frames_for(&[&layout]);

		let (modification, modification_layout) = lifted_value(ContextModification::from_sources(ContextFeatures::all(), &[]));
		let node = crate::context_modification::ContextModificationNode::new(
			RecordSourceNode {
				layout: layout.clone(),
				element: 4.,
				fields: vec![("opacity", 0.25)],
				partial: true,
			},
			modification,
			&layout,
			&modification_layout,
		);

		let GPoll::Partial(served) = core_types::record::capture(&node, &ctx, &frames) else {
			panic!("expected a partial record");
		};
		assert_eq!(served.element::<f64>(), 4.);
		assert_eq!(served.field::<f64>("opacity", 0), 0.25);
	}

	#[test]
	fn droppable_elements_park_and_clone_out() {
		let frames = core_types::record::test_frames(1 << 16);
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let lift = ValueSource::new(String::from("parked"));
		let layout = Node::<ContextImpl>::layout(&lift).clone();
		let chain = core_types::record::RecordExtract::<String, _>::new(lift, &layout);

		let GPoll::Final(text) = chain.eval(&ctx, &frames) else {
			panic!("expected a final value");
		};
		assert_eq!(text, "parked");
	}

	#[test]
	fn identity_layouts_forward_the_record_pointer() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = f64_layout(&["opacity", "length"]);
		let frames = frames_for(&[&layout]);
		let base = {
			let probe = frames.scope();
			// SAFETY: nothing reads the probe's record; only its address is taken.
			let value = unsafe { probe.claim(&layout).finish() };
			layout.rec(&value).ptr()
		};

		let chain = ForwardRecordNode::new(RecordSource::new(f64_record_source(&layout, 4., vec![("opacity", 0.25)]), &layout, &layout.clone()), &layout);

		let GPoll::Final(value) = core_types::record::serve_input(&chain, &ctx, &frames) else {
			panic!("expected a final record");
		};
		let rec = layout.rec(&value);
		assert_eq!(rec.ptr(), base, "the served record's pointer is the claimed slot's");
		assert_eq!(unsafe { rec.element::<f64>() }, 4.);
		assert_eq!(unsafe { rec.read::<f64>(layout.offset_of("opacity", 0).unwrap()) }, 0.25);
	}

	#[test]
	fn partial_routing_sources_downgrade_the_output() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = f64_layout(&["opacity"]);
		let frames = frames_for(&[&layout]);

		let chain = ForwardRecordNode::new(
			RecordSource::new(
				RecordSourceNode {
					layout: layout.clone(),
					element: 4.,
					fields: vec![],
					partial: true,
				},
				&layout,
				&layout.clone(),
			),
			&layout,
		);

		let GPoll::Partial(served) = core_types::record::capture(&chain, &ctx, &frames) else {
			panic!("expected a partial record");
		};
		assert_eq!(served.element::<f64>(), 4.);
	}

	fn counting_source(evals: std::sync::Arc<std::sync::atomic::AtomicU32>) -> LiftedSource<f64, impl for<'c> Fn(&ContextImpl<'c>) -> GPoll<f64>> {
		LiftedSource::new(move |_: &ContextImpl<'_>| {
			evals.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
			GPoll::Final(21.)
		})
	}

	#[test]
	fn record_memo_serves_a_context_hit_without_re_evaluating() {
		let frames = core_types::record::test_frames(1 << 16);
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let evals = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
		let lift = counting_source(evals.clone());
		let layout = Node::<ContextImpl>::layout(&lift).clone();
		let memo = crate::memo::MemoizeNode::new(lift, &layout);

		let GPoll::Final(served) = core_types::record::capture(&memo, &ctx, &frames) else {
			panic!("expected a final record");
		};
		assert_eq!(served.element::<f64>(), 21.);
		let GPoll::Final(served) = core_types::record::capture(&memo, &ctx, &frames) else {
			panic!("expected a final record");
		};
		assert_eq!(served.element::<f64>(), 21.);
		assert_eq!(evals.load(std::sync::atomic::Ordering::Relaxed), 1, "a context hit must not re-evaluate the edge");
	}

	#[test]
	fn record_memo_caches_partial_finality() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = f64_layout(&["opacity"]);
		let frames = frames_for(&[&layout, &layout]);

		let source = RecordSourceNode {
			layout: layout.clone(),
			element: 4.,
			fields: vec![("opacity", 0.5)],
			partial: true,
		};
		let memo = crate::memo::MemoizeNode::new(source, &layout);

		let GPoll::Partial(_) = core_types::record::serve_input(&memo, &ctx, &frames) else {
			panic!("expected a partial record");
		};
		let GPoll::Partial(served) = core_types::record::capture(&memo, &ctx, &frames) else {
			panic!("expected the hit to keep the partial finality");
		};
		assert_eq!(served.field::<f64>("opacity", 0), 0.5);
	}

	#[test]
	fn record_memo_serves_droppable_payloads_from_the_persistent_region() {
		let generations = [];
		let persistent = Arena::new(1024).unwrap();

		let source_layout = f64_layout(&[]);
		let labeled = label_layout(&source_layout);
		let frames = frames_for(&[&labeled, &labeled]);

		let chain = install(
			LabelNode::new(bare_source(&source_layout, 1.), ValueSource::new(String::from("a")), &source_layout),
			label_layout_meta(),
			&[Some(&source_layout)],
		);
		let memo = crate::memo::MemoizeNode::new(chain, &labeled);

		let first_arena = Arena::new(1024).unwrap();
		{
			let scope = scope_fixture(&generations, &first_arena).with_persistent(&persistent);
			let ctx = ContextImpl::root(&scope);
			let mut first = frames.reborrow();
			let GPoll::Final(_) = core_types::record::serve_input(&memo, &ctx, &mut first) else {
				panic!("expected a final record");
			};
		}
		// The evaluation that published is gone; only the promotion survives.
		drop(first_arena);

		let later_arena = Arena::new(1024).unwrap();
		let scope = scope_fixture(&generations, &later_arena).with_persistent(&persistent);
		let ctx = ContextImpl::root(&scope);
		let GPoll::Final(value) = core_types::record::serve_input(&memo, &ctx, &frames) else {
			panic!("expected a final record");
		};
		let rec = labeled.rec(&value);
		assert_eq!(unsafe { rec.element::<f64>() }, 1.);
		assert_eq!(unsafe { rec.read::<&str>(labeled.offset_of(Label::NAME, 0).unwrap()) }, "a");
	}
}
