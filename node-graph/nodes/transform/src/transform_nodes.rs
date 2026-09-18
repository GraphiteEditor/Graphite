use core::f64;
use core_types::attribute::{Attr, Transform as TransformAttr};
use core_types::color::Color;
use core_types::extent::{ExtentIn, LevelIn, ValueIn};
use core_types::gpoll::{Extent, GPoll, Interrupt};
use core_types::transform::{ApplyTransform, ScaleType, Transform};
use core_types::{CacheHash, Context, Ctx, DeriveCtx, InjectFootprint, ModifyFootprint};
use glam::{DAffine2, DMat2, DVec2};
use graphic_types::raster_types::{CPU, GPU, Raster};
use graphic_types::{Artboard, Graphic, Vector};
use vector_types::Gradient;

/// Applies the specified transform to each lane of the input, composing onto the lane's transform attribute.
#[node_macro::node(category("Math: Transform"), extent(transform_extent), batch(transform_batch))]
fn transform<T>(
	ctx: impl Ctx + DeriveCtx + ModifyFootprint,
	content: impl Node<Context<'_>, Output = (T, Attr<TransformAttr>)>,
	#[widget(ParsedWidgetOverride::Custom = "transform_translation")] translation: DVec2,
	#[widget(ParsedWidgetOverride::Custom = "transform_rotation")] rotation: f64,
	#[widget(ParsedWidgetOverride::Custom = "transform_scale")]
	#[default(1., 1.)]
	scale: DVec2,
	#[widget(ParsedWidgetOverride::Custom = "transform_skew")] skew: DVec2,
) -> Result<(T, Attr<TransformAttr>), Interrupt> {
	let trs = DAffine2::from_scale_angle_translation(scale, rotation.to_radians(), translation);
	let skew = DAffine2::from_cols_array(&[1., skew.y.to_radians().tan(), skew.x.to_radians().tan(), 1., 0., 0.]);
	let matrix = trs * skew;

	let transformed = ctx.modify_footprint(|footprint| footprint.apply_transform(&matrix));
	let (element, transform) = content.eval(&transformed.ctx())?;

	Ok((element, Attr(matrix * *transform)))
}

fn transform_extent(content: ExtentIn<'_>, _translation: ValueIn<'_, DVec2>, _rotation: ValueIn<'_, f64>, _scale: ValueIn<'_, DVec2>, _skew: ValueIn<'_, DVec2>, level: LevelIn) -> GPoll<Extent> {
	content.at(level)
}

/// The transform applied to a plain transform or point value. Registered under the same identifier
/// as the leveled `transform`, serving its value-typed rows.
#[node_macro::node(category(""))]
fn transform_value<T: ApplyTransform + 'static>(
	ctx: impl Ctx + DeriveCtx + ModifyFootprint,
	#[implementations(Context -> DAffine2, Context -> DVec2)] content: impl Node<Context<'_>, Output = T>,
	#[widget(ParsedWidgetOverride::Custom = "transform_translation")] translation: DVec2,
	#[widget(ParsedWidgetOverride::Custom = "transform_rotation")] rotation: f64,
	#[widget(ParsedWidgetOverride::Custom = "transform_scale")]
	#[default(1., 1.)]
	scale: DVec2,
	#[widget(ParsedWidgetOverride::Custom = "transform_skew")] skew: DVec2,
) -> Result<T, Interrupt> {
	let trs = DAffine2::from_scale_angle_translation(scale, rotation.to_radians(), translation);
	let skew = DAffine2::from_cols_array(&[1., skew.y.to_radians().tan(), skew.x.to_radians().tan(), 1., 0., 0.]);
	let matrix = trs * skew;

	let transformed = ctx.modify_footprint(|footprint| footprint.apply_transform(&matrix));
	let mut transform_target = content.eval(&transformed.ctx())?;

	transform_target.left_apply_transform(&matrix);

	Ok(transform_target)
}

pub use _transform_value_mod::transform_value_entries;

/// Resets the desired components of the input transform to their default values. If all components are reset, the output will be set to the identity transform.
/// Shear is represented jointly by rotation and scale, so resetting both will also remove any shear.
#[node_macro::node(category("Math: Transform"))]
fn reset_transform<T>(_: impl Ctx, (element, transform): (T, Attr<TransformAttr>), #[default(true)] reset_translation: bool, reset_rotation: bool, reset_scale: bool) -> (T, Attr<TransformAttr>) {
	let mut row_transform = *transform;
	if reset_translation {
		row_transform.translation = DVec2::ZERO;
	}

	match (reset_rotation, reset_scale) {
		(true, true) => row_transform.matrix2 = DMat2::IDENTITY,
		(true, false) => {
			let scale = row_transform.scale_magnitudes();
			row_transform.matrix2 = DMat2::from_diagonal(scale);
		}
		(false, true) => {
			let rotation = row_transform.decompose_rotation();
			row_transform.matrix2 = DMat2::from_angle(rotation);
		}
		(false, false) => {}
	}
	(element, Attr(row_transform))
}

/// Overwrites the transform of each lane of the input with the specified transform.
#[node_macro::node(category("Math: Transform"))]
fn replace_transform<T>(_: impl Ctx + InjectFootprint, (element, _content_transform): (T, Attr<TransformAttr>), transform: DAffine2) -> (T, Attr<TransformAttr>) {
	(element, Attr(transform))
}

// TODO: Figure out how this node should behave once #2982 is implemented.
/// Obtains the transform of the first lane of the input, if present.
#[node_macro::node(category("Math: Transform"), path(core_types::vector))]
fn extract_transform<T: Clone + Send + Sync + CacheHash + 'static>(
	_: impl Ctx,
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Artboard)] content: IList<T>,
) -> DAffine2 {
	match content.len() {
		0 => DAffine2::default(),
		_ => content.lane(0).attr::<TransformAttr>(),
	}
}

/// Produces the inverse of the input transform, which is the transform that undoes the effect of the original transform.
#[node_macro::node(category("Math: Transform"))]
fn invert_transform(_: impl Ctx, transform: DAffine2) -> DAffine2 {
	transform.inverse()
}

/// Extracts the translation component from the input transform.
#[node_macro::node(category("Math: Transform"))]
fn decompose_translation(_: impl Ctx, transform: DAffine2) -> DVec2 {
	transform.translation
}

/// Extracts the rotation component (in degrees) from the input transform.
#[node_macro::node(category("Math: Transform"))]
fn decompose_rotation(_: impl Ctx, transform: DAffine2) -> f64 {
	transform.decompose_rotation().to_degrees()
}

/// Extracts the scale component from the input transform.
/// **Magnitude** returns the visual length of each axis (always positive, includes any skew contribution).
/// **Pure** returns the isolated scale factors with rotation and skew stripped away (can be negative for flipped axes).
#[node_macro::node(category("Math: Transform"))]
fn decompose_scale(_: impl Ctx, transform: DAffine2, scale_type: ScaleType) -> DVec2 {
	match scale_type {
		ScaleType::Magnitude => transform.scale_magnitudes(),
		ScaleType::Pure => transform.decompose_scale(),
	}
}

/// Extracts the skew angle (in degrees) from the input transform.
#[node_macro::node(category("Math: Transform"))]
fn decompose_skew(_: impl Ctx, transform: DAffine2) -> f64 {
	transform.decompose_skew().atan().to_degrees()
}

/// The batch through the transform: the footprint derives once for the whole
/// range, the content answers the range through the caller's scratch, and
/// each lane's transform attribute is composed in place.
fn transform_batch<'batch, 'serve, 'r, Input, Content, Translation, Rotation, Scale, Skew>(
	node: &'batch _transform_mod::TransformNode<Content, Translation, Rotation, Scale, Skew>,
	dispatch: core_types::dispatch::Dispatch<'serve>,
	scratch: Option<&'r mut [std::mem::MaybeUninit<u64>]>,
	frames: &core_types::record::Frames<'serve>,
) -> core_types::node::BatchStatus<'r>
where
	'batch: 'r,
	'serve: 'r,
	Input:
		Ctx + DeriveCtx + ModifyFootprint + core_types::InjectIndex + Copy + core_types::dispatch::AsDispatch<'serve> + core_types::context::ExtractArena<ArenaRef = &'serve core_types::arena::Arena>,
	Content: for<'derived> core_types::record::DerivedRecordInput<'derived, core_types::context::Derived<'derived, Input>>,
	Translation: core_types::node::Node<Input>,
	Rotation: core_types::node::Node<Input>,
	Scale: core_types::node::Node<Input>,
	Skew: core_types::node::Node<Input>,
	_transform_mod::TransformNode<Content, Translation, Rotation, Scale, Skew>: core_types::node::Node<Input>,
{
	use core_types::node::BatchStatus;

	let range = dispatch.range();
	let Ok(len) = usize::try_from(range.end.saturating_sub(range.start)) else {
		return BatchStatus::InvalidRange;
	};
	// A value input binds once only where it is invariant over every level the
	// map varies. Content that reads the footprint sees one footprint per
	// batch, so lane-varying values leave it to the lane loop.
	let hoists = |input: usize| dispatch.map().depth() == 1 && node.__lane_invariant & (1 << input) != 0;
	let invariant = (1..=4).all(hoists);
	if !invariant && node.__footprint_free & 1 == 0 {
		return BatchStatus::Unbatched;
	}
	let cell = core_types::node::StatusCell::new();
	let translation = match lane_values::<DVec2, Input, _>(1, &node.translation, &dispatch, hoists(1), &cell, frames) {
		Ok(values) => values,
		Err(status) => return status,
	};
	let rotation = match lane_values::<f64, Input, _>(2, &node.rotation, &dispatch, hoists(2), &cell, frames) {
		Ok(values) => values,
		Err(status) => return status,
	};
	let scale = match lane_values::<DVec2, Input, _>(3, &node.scale, &dispatch, hoists(3), &cell, frames) {
		Ok(values) => values,
		Err(status) => return status,
	};
	let skew = match lane_values::<DVec2, Input, _>(4, &node.skew, &dispatch, hoists(4), &cell, frames) {
		Ok(values) => values,
		Err(status) => return status,
	};
	let len = [translation.len(), rotation.len(), scale.len(), skew.len()].into_iter().flatten().fold(len, usize::min);
	let matrix_at = |lane: usize| {
		// SAFETY: each value batch came from the input whose layout it is read at.
		let (translation, rotation, scale, skew) = unsafe { (translation.at(lane), rotation.at(lane), scale.at(lane), skew.at(lane)) };
		let trs = DAffine2::from_scale_angle_translation(scale, rotation.to_radians(), translation);
		let skew = DAffine2::from_cols_array(&[1., skew.y.to_radians().tan(), skew.x.to_radians().tan(), 1., 0., 0.]);
		trs * skew
	};
	let footprint = match invariant {
		true => {
			let Some(base) = Input::at_lane(&dispatch, range.start) else {
				return exhausted();
			};
			base.try_footprint().copied().map(|mut footprint| {
				footprint.apply_transform(&matrix_at(0));
				footprint
			})
		}
		false => None,
	};
	let derived = match &footprint {
		Some(footprint) => dispatch.with_footprint(footprint),
		None => dispatch.clone(),
	};
	let layout = core_types::node::Node::<Input>::layout(node);
	let carrier = &node.__carrier;
	let Some(offset) = core_types::record::FieldOffset::<TransformAttr>::of(layout, 0).map(|field| field.offset()) else {
		return BatchStatus::Error(core_types::gpoll::GraphError::new("the transform layout carries no transform column"));
	};
	// Content without a transform column composes onto the attribute default.
	let source_offset = core_types::record::FieldOffset::<TransformAttr>::of(carrier, 0).map(|field| field.offset());
	#[cfg(debug_assertions)]
	core_types::record::note_kernel_batch("transform", if invariant { "translate" } else { "translate varying" }, len);
	let arena = dispatch.scope().arena();
	// The content serves lane by lane at its own layout when it has no batch.
	let serve_lanes = |scratch: &'r mut [std::mem::MaybeUninit<u64>]| {
		use core_types::gpoll::Finality;
		let Some(mut run) = frames.run(scratch, len, carrier) else {
			return BatchStatus::InvalidRange;
		};
		let mut finality = Finality::AllFinal;
		let mut hint = Extent::AtLeast(range.end as usize);
		for lane in 0..len {
			let Some(ctx) = <core_types::context::Derived<'_, Input> as core_types::dispatch::AsDispatch<'_>>::at_lane(&derived, range.start + lane as u64) else {
				return exhausted();
			};
			let lane_frames = frames.scope();
			let slot = run.slot(lane, &lane_frames);
			let served = match node.content.serve_derived(&ctx, slot) {
				GPoll::Final(served) => served,
				GPoll::Partial(served) => {
					finality = Finality::Partial;
					served
				}
				GPoll::Pending => return BatchStatus::Pending,
				GPoll::Fallback(boxed) => return BatchStatus::Error(boxed.1),
				GPoll::Error(error) if error.kind == core_types::gpoll::ErrorKind::PastEnd => {
					hint = Extent::Exactly(range.start as usize + lane);
					break;
				}
				GPoll::Error(error) => return BatchStatus::Error(*error),
			};
			run.served(lane, &served);
		}
		BatchStatus::Filled(run.finish(), finality, hint)
	};
	let words = len * carrier.lane_stride() / 8;
	let (source, finality, hint) = match node.content.eval_batch_derived(derived.over(range.start..range.start + len as u64), None, frames) {
		BatchStatus::Lent(batch, finality, hint) => (batch, finality, hint),
		BatchStatus::NeedBuffer | BatchStatus::Unbatched if scratch.is_none() => return BatchStatus::NeedBuffer,
		status @ (BatchStatus::NeedBuffer | BatchStatus::Unbatched) => {
			let Some(owned) = arena.alloc_scratch::<u64>(words) else {
				return exhausted();
			};
			let filled = match status {
				BatchStatus::NeedBuffer => node.content.eval_batch_derived(derived.over(range.start..range.start + len as u64), Some(owned), frames),
				_ => serve_lanes(owned),
			};
			match filled {
				BatchStatus::Filled(batch, finality, hint) => (batch.into_shared(), finality, hint),
				BatchStatus::Pending => return BatchStatus::Pending,
				BatchStatus::Error(error) => return BatchStatus::Error(error),
				BatchStatus::InvalidRange => return BatchStatus::InvalidRange,
				_ => return BatchStatus::Error(core_types::gpoll::GraphError::new("the transform content answered no batch")),
			}
		}
		BatchStatus::Pending => return BatchStatus::Pending,
		BatchStatus::Error(error) => return BatchStatus::Error(error),
		BatchStatus::InvalidRange => return BatchStatus::InvalidRange,
		BatchStatus::Filled(..) => return BatchStatus::Error(core_types::gpoll::GraphError::new("a batch filled scratch it was not given")),
	};
	let Some(scratch) = scratch else {
		return BatchStatus::NeedBuffer;
	};
	let stride = layout.lane_stride();
	if scratch.len() * 8 < len * stride {
		return BatchStatus::InvalidRange;
	}
	let lanes = source.len().min(len);
	let base: *mut u8 = scratch.as_mut_ptr().cast();
	for lane in 0..lanes {
		let src = source.get(lane).rec();
		// SAFETY: the source lane is a record of the carrier layout, the scratch
		// holds `len` lanes of this layout, and the plan maps carrier fields to
		// their offsets here; the transform column is the one write left.
		unsafe {
			let dst = base.add(lane * stride);
			core_types::record::apply_plan(src, dst, &node.__plan);
			let current = match source_offset {
				Some(source_offset) => src.ptr().add(source_offset).cast::<DAffine2>().read_unaligned(),
				None => <TransformAttr as core_types::attribute::Attribute>::default(),
			};
			core_types::record::write_field(dst, offset, matrix_at(lane) * current);
		}
	}
	BatchStatus::Filled(core_types::node::RecordBatchMut::filled(scratch, lanes, layout), finality, hint)
}

fn exhausted<'r>() -> core_types::node::BatchStatus<'r> {
	core_types::node::BatchStatus::Error(core_types::gpoll::GraphError {
		kind: core_types::gpoll::ErrorKind::ArenaExhausted,
		trace: Vec::new(),
	})
}

/// A value input over a batch: bound once at the base lane, or one record per lane.
enum LaneValues<'r, T> {
	One(T),
	Each(core_types::node::RecordBatch<'r>, std::marker::PhantomData<T>),
}

impl<T: Clone> LaneValues<'_, T> {
	fn len(&self) -> Option<usize> {
		match self {
			Self::One(_) => None,
			Self::Each(batch, _) => Some(batch.len()),
		}
	}

	/// # Safety
	/// The batch's lanes are records of the layout `T` was resolved at.
	unsafe fn at(&self, lane: usize) -> T {
		match self {
			Self::One(value) => value.clone(),
			// SAFETY: the caller's contract.
			Self::Each(batch, _) => unsafe { core_types::record::read_element::<T>(batch.get(lane).rec()) },
		}
	}
}

fn lane_values<'a, 'e, 'r, T, C, N>(
	input: usize,
	node: &'a N,
	dispatch: &core_types::dispatch::Dispatch<'e>,
	hoist: bool,
	cell: &core_types::node::StatusCell,
	frames: &core_types::record::Frames<'e>,
) -> Result<LaneValues<'r, T>, core_types::node::BatchStatus<'r>>
where
	'a: 'r,
	'e: 'r,
	T: Clone,
	C: core_types::dispatch::AsDispatch<'e> + core_types::InjectIndex + Copy + core_types::context::ExtractArena<ArenaRef = &'e core_types::arena::Arena>,
	N: core_types::node::Node<C>,
{
	use core_types::node::BatchStatus;
	if hoist {
		let Some(base) = C::at_lane(dispatch, dispatch.range().start) else {
			return Err(exhausted());
		};
		let value = cell.eval_input(input, node, &base, frames).map_err(BatchStatus::from)?;
		// SAFETY: the value came from `node`, so it is a record of its layout.
		return Ok(LaneValues::One(unsafe { core_types::record::read_element::<T>(core_types::node::Node::<C>::layout(node).rec(&value)) }));
	}
	match core_types::record::materialize_dispatch::<C, N>(node, dispatch, dispatch.scope().arena(), frames) {
		BatchStatus::Lent(batch, ..) => Ok(LaneValues::Each(batch, std::marker::PhantomData)),
		BatchStatus::Filled(batch, ..) => Ok(LaneValues::Each(batch.into_shared(), std::marker::PhantomData)),
		BatchStatus::Pending => Err(BatchStatus::Pending),
		BatchStatus::Error(error) => Err(BatchStatus::Error(error)),
		BatchStatus::InvalidRange => Err(BatchStatus::InvalidRange),
		_ => Err(BatchStatus::Error(core_types::gpoll::GraphError::new("a value input answered no batch"))),
	}
}
