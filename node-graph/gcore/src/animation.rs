use crate::{Ctx, ExtractFrameIndex, ExtractFrameTime, ExtractTime};

const DAY: f64 = 1000. * 3600. * 24.;

#[node_macro::node(category("Animation"))]
fn time_utc(ctx: impl Ctx + ExtractTime) -> f64 {
	ctx.try_time().unwrap_or_default()
}
#[node_macro::node(category("Animation"))]
fn year(ctx: impl Ctx + ExtractTime) -> f64 {
	(ctx.try_time().unwrap_or_default() / DAY / 365.25).floor() + 1970.
}
// #[node_macro::node(category("Animation"))]
// fn month(ctx: impl Ctx + ExtractTime) -> f64 {
// 	(ctx.try_time().unwrap_or_default() / DAY % 12.).floor()
// }
// #[node_macro::node(category("Animation"))]
// fn day(ctx: impl Ctx + ExtractTime) -> f64 {
// 	(ctx.try_time().unwrap_or_default() / DAY
// }
#[node_macro::node(category("Animation"))]
fn hour(ctx: impl Ctx + ExtractTime) -> f64 {
	(ctx.try_time().unwrap_or_default() / 1000. / 3600.).floor() % 24.
}
#[node_macro::node(category("Animation"))]
fn minute(ctx: impl Ctx + ExtractTime) -> f64 {
	(ctx.try_time().unwrap_or_default() / 1000. / 60.).floor() % 60.
}
#[node_macro::node(category("Animation"))]
fn second(ctx: impl Ctx + ExtractTime) -> f64 {
	(ctx.try_time().unwrap_or_default() / 1000.).floor() % 60.
}
#[node_macro::node(category("Animation"))]
fn millisecond(ctx: impl Ctx + ExtractTime) -> f64 {
	ctx.try_time().unwrap_or_default() % 1000.
}

#[node_macro::node(category("Animation"))]
fn frame(ctx: impl Ctx + ExtractFrameIndex) -> f64 {
	ctx.try_frame_index().unwrap_or_default()
}
#[node_macro::node(category("Animation"))]
fn frame_time(ctx: impl Ctx + ExtractFrameTime) -> f64 {
	ctx.try_frame_time().unwrap_or_default()
}
