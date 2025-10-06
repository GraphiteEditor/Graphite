use crate::{Ctx, ExtractAnimationTime, ExtractRealTime};

const DAY: f64 = 1000. * 3600. * 24.;

#[derive(Debug, Clone, Copy, PartialEq, Eq, dyn_any::DynAny, Default, Hash, node_macro::ChoiceType, serde::Serialize, serde::Deserialize)]
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
fn real_time(ctx: impl Ctx + ExtractRealTime, _primary: (), mode: RealTimeMode) -> f64 {
	let real_time = ctx.try_real_time().unwrap_or_default();
	// TODO: Implement proper conversion using and existing time implementation
	match mode {
		RealTimeMode::Utc => real_time,
		RealTimeMode::Year => (real_time / DAY / 365.25).floor() + 1970.,
		RealTimeMode::Hour => (real_time / 1000. / 3600.).floor() % 24., // TODO: Factor in a chosen timezone
		RealTimeMode::Minute => (real_time / 1000. / 60.).floor() % 60., // TODO: Factor in a chosen timezone

		RealTimeMode::Second => (real_time / 1000.).floor() % 60.,
		RealTimeMode::Millisecond => real_time % 1000.,
	}
}

/// Produces the time, in seconds on the timeline, since the beginning of animation playback.
#[node_macro::node(category("Animation"))]
fn animation_time(ctx: impl Ctx + ExtractAnimationTime) -> f64 {
	ctx.try_animation_time().unwrap_or_default()
}

// These nodes require more sophisticated algorithms for giving the correct result

// #[node_macro::node(category("Animation"))]
// fn month(ctx: impl Ctx + ExtractRealTime) -> f64 {
// 	((ctx.try_real_time().unwrap_or_default() / DAY / 365.25 % 1.) * 12.).floor()
// }
// #[node_macro::node(category("Animation"))]
// fn day(ctx: impl Ctx + ExtractRealTime) -> f64 {
// 	(ctx.try_real_time().unwrap_or_default() / DAY
// }
