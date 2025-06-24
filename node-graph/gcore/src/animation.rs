use crate::{Ctx, ExtractAnimationTime, ExtractTime};

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

#[node_macro::node(category("Animation"))]
fn real_time(ctx: impl Ctx + ExtractTime, _primary: (), mode: RealTimeMode) -> f64 {
	let time = ctx.try_time().unwrap_or_default();
	// TODO: Implement proper conversion using and existing time implementation
	match mode {
		RealTimeMode::Utc => time,
		RealTimeMode::Year => (time / DAY / 365.25).floor() + 1970.,
		RealTimeMode::Hour => (time / 1000. / 3600.).floor() % 24.,
		RealTimeMode::Minute => (time / 1000. / 60.).floor() % 60.,

		RealTimeMode::Second => (time / 1000.).floor() % 60.,
		RealTimeMode::Millisecond => time % 1000.,
	}
}

#[node_macro::node(category("Animation"))]
fn animation_time(ctx: impl Ctx + ExtractAnimationTime) -> f64 {
	ctx.try_animation_time().unwrap_or_default()
}

// These nodes require more sophistcated algorithms for giving the correct result

// #[node_macro::node(category("Animation"))]
// fn month(ctx: impl Ctx + ExtractTime) -> f64 {
// 	((ctx.try_time().unwrap_or_default() / DAY / 365.25 % 1.) * 12.).floor()
// }
// #[node_macro::node(category("Animation"))]
// fn day(ctx: impl Ctx + ExtractTime) -> f64 {
// 	(ctx.try_time().unwrap_or_default() / DAY
// }
