use core_types::list::Item;
use core_types::transform::Footprint;
use core_types::{CacheHash, CloneVarArgs, Color, Context, Ctx, ExtractAll, ExtractAnimationTime, ExtractPointerPosition, ExtractRealTime, OwnedContextImpl};
use glam::{DAffine2, DVec2};
use graphic_types::vector_types::GradientStops;
use graphic_types::{Graphic, Vector};
use raster_types::{CPU, GPU, Raster};

const DAY: f64 = 1000. * 3600. * 24.;

#[derive(Debug, Clone, Copy, PartialEq, Eq, dyn_any::DynAny, Default, Hash, CacheHash, node_macro::ChoiceType)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RealTimeMode {
	#[label("UTC")]
	Utc,
	Year,
	Hour,
	Minute,
	#[default]
	Second,
	Millisecond,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationTimeMode {
	AnimationTime,
	FrameNumber,
}

/// Produces a chosen representation of the current real time and date (in UTC) based on the system clock.
#[node_macro::node(category("Animation"))]
fn real_time(
	ctx: impl Ctx + ExtractRealTime,
	_primary: (),
	/// The time and date component to be produced as a number.
	component: RealTimeMode,
) -> Item<f64> {
	let real_time = ctx.try_real_time().unwrap_or_default();

	// TODO: Implement proper conversion using and existing time implementation
	let result = match component {
		RealTimeMode::Utc => real_time,
		RealTimeMode::Year => (real_time / DAY / 365.25).floor() + 1970., // TODO: Factor in a chosen timezone
		RealTimeMode::Hour => (real_time / 1000. / 3600.).floor() % 24.,  // TODO: Factor in a chosen timezone
		RealTimeMode::Minute => (real_time / 1000. / 60.).floor() % 60.,  // TODO: Factor in a chosen timezone
		RealTimeMode::Second => (real_time / 1000.).floor() % 60.,
		RealTimeMode::Millisecond => real_time % 1000.,
	};

	Item::new_from_element(result)
}

/// Produces the time, in seconds on the timeline, since the beginning of animation playback.
#[node_macro::node(category("Animation"))]
fn animation_time(
	ctx: impl Ctx + ExtractAnimationTime,
	_primary: (),
	#[default(1)]
	#[unit("/sec")]
	rate: f64,
) -> Item<f64> {
	Item::new_from_element(ctx.try_animation_time().unwrap_or_default() * rate)
}

#[node_macro::node(category("Debug"))]
async fn quantize_real_time<T>(
	ctx: impl Ctx + ExtractAll + CloneVarArgs,
	#[implementations(
		Context -> Item<bool>,
		Context -> Item<u32>,
		Context -> Item<u64>,
		Context -> Item<f32>,
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
		Context -> Item<GradientStops>,
	)]
	value: impl Node<'n, Context<'static>, Output = T>,
	#[default(1)]
	#[unit("sec")]
	quantum: Item<f64>,
) -> T {
	let time = ctx.try_real_time().unwrap_or_default();
	let time = time / 1000.;
	let quantum = quantum.into_element();
	let mut quantized_time = (time * quantum.recip()).round() / quantum.recip();
	if !quantized_time.is_finite() {
		quantized_time = time;
	}
	let quantized_time = quantized_time * 1000.;
	let new_context = OwnedContextImpl::from(ctx).with_real_time(quantized_time);
	value.eval(Some(new_context.into())).await
}

#[node_macro::node(category("Debug"))]
async fn quantize_animation_time<T>(
	ctx: impl Ctx + ExtractAll + CloneVarArgs,
	#[implementations(
		Context -> Item<bool>,
		Context -> Item<u32>,
		Context -> Item<u64>,
		Context -> Item<f32>,
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
		Context -> Item<GradientStops>,
	)]
	value: impl Node<'n, Context<'static>, Output = T>,
	#[default(1)]
	#[unit("sec")]
	quantum: Item<f64>,
) -> T {
	let time = ctx.try_animation_time().unwrap_or_default();
	let quantum = quantum.into_element();
	let mut quantized_time = (time * quantum.recip()).round() / quantum.recip();
	if !quantized_time.is_finite() {
		quantized_time = time;
	}
	let new_context = OwnedContextImpl::from(ctx).with_animation_time(quantized_time);
	value.eval(Some(new_context.into())).await
}

/// Produces the current position of the user's pointer within the document canvas.
#[node_macro::node(category("Animation"))]
fn pointer_position(ctx: impl Ctx + ExtractPointerPosition) -> Item<DVec2> {
	Item::new_from_element(ctx.try_pointer_position().unwrap_or_default())
}

// TODO: These nodes require more sophisticated algorithms for giving the correct result
// #[node_macro::node(category("Animation"))]
// fn month(ctx: impl Ctx + ExtractRealTime) -> f64 {
// 	((ctx.try_real_time().unwrap_or_default() / DAY / 365.25 % 1.) * 12.).floor()
// }
// #[node_macro::node(category("Animation"))]
// fn day(ctx: impl Ctx + ExtractRealTime) -> f64 {
// 	(ctx.try_real_time().unwrap_or_default() / DAY
// }
