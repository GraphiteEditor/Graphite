use crate::Artboard;
use crate::Color;
use crate::Graphic;
use crate::gradient::GradientStops;
use crate::graphene_core::registry::types::TextArea;
use crate::raster_types::{CPU, GPU, Raster};
use crate::table::Table;
use crate::vector::Vector;
use crate::{Context, Ctx};
use glam::{DAffine2, DVec2};

#[node_macro::node(category("Type Conversion"))]
fn to_string<T: std::fmt::Debug>(_: impl Ctx, #[implementations(String, bool, f64, u32, u64, DVec2, DAffine2, Table<Vector>)] value: T) -> String {
	format!("{:?}", value)
}

#[node_macro::node(category("Text"))]
fn serialize<T: serde::Serialize>(
	_: impl Ctx,
	#[implementations(String, bool, f64, u32, u64, DVec2, DAffine2, Color, Option<Color>, Table<Graphic>, Table<Vector>, Table<Raster<CPU>>)] value: T,
) -> String {
	serde_json::to_string(&value).unwrap_or_else(|_| "Serialization Error".to_string())
}

#[node_macro::node(category("Text"))]
fn string_concatenate(_: impl Ctx, #[implementations(String)] first: String, second: TextArea) -> String {
	first.clone() + &second
}

#[node_macro::node(category("Text"))]
fn string_replace(_: impl Ctx, #[implementations(String)] string: String, from: TextArea, to: TextArea) -> String {
	string.replace(&from, &to)
}

#[node_macro::node(category("Text"))]
fn string_slice(_: impl Ctx, #[implementations(String)] string: String, start: f64, end: f64) -> String {
	let start = if start < 0. { string.len() - start.abs() as usize } else { start as usize };
	let end = if end <= 0. { string.len() - end.abs() as usize } else { end as usize };
	let n = end.saturating_sub(start);
	string.char_indices().skip(start).take(n).map(|(_, c)| c).collect()
}

#[node_macro::node(category("Text"))]
fn string_length(_: impl Ctx, #[implementations(String)] string: String) -> u32 {
	string.chars().count() as u32
}

#[node_macro::node(category("Math: Logic"))]
async fn switch<T, C: Send + 'n + Clone>(
	#[implementations(Context)] ctx: C,
	condition: bool,
	#[expose]
	#[implementations(
		Context -> String,
		Context -> bool,
		Context -> f32,
		Context -> f64,
		Context -> u32,
		Context -> u64,
		Context -> DVec2,
		Context -> DAffine2,
		Context -> Table<Artboard>,
		Context -> Table<Graphic>,
		Context -> Table<Vector>,
		Context -> Table<Raster<CPU>>,
		Context -> Table<Raster<GPU>>,
		Context -> Color,
		Context -> Option<Color>,
		Context -> GradientStops,
	)]
	if_true: impl Node<C, Output = T>,
	#[expose]
	#[implementations(
		Context -> String,
		Context -> bool,
		Context -> f32,
		Context -> f64,
		Context -> u32,
		Context -> u64,
		Context -> DVec2,
		Context -> DAffine2,
		Context -> Table<Artboard>,
		Context -> Table<Graphic>,
		Context -> Table<Vector>,
		Context -> Table<Raster<CPU>>,
		Context -> Table<Raster<GPU>>,
		Context -> Color,
		Context -> Option<Color>,
		Context -> GradientStops,
	)]
	if_false: impl Node<C, Output = T>,
) -> T {
	if condition {
		// We can't remove these calls because we only want to evaluate the branch that we actually need
		if_true.eval(ctx).await
	} else {
		if_false.eval(ctx).await
	}
}
