use crate::vector::VectorDataTable;
use crate::{Context, Ctx};
use glam::{DAffine2, DVec2};

/// The equality operation (==) compares two values and returns true if they are equal, or false if they are not.
#[node_macro::node(category("Math: Logic"))]
fn equals<U: core::cmp::PartialEq<T>, T>(
	_: impl Ctx,
	#[implementations(f64, &f64, f32, &f32, u32, &u32, DVec2, &DVec2, &str)] value: T,
	#[implementations(f64, &f64, f32, &f32, u32, &u32, DVec2, &DVec2, &str)] other_value: U,
) -> bool {
	other_value == value
}

/// The inequality operation (!=) compares two values and returns true if they are not equal, or false if they are.
#[node_macro::node(category("Math: Logic"))]
fn not_equals<U: core::cmp::PartialEq<T>, T>(
	_: impl Ctx,
	#[implementations(f64, &f64, f32, &f32, u32, &u32, DVec2, &DVec2, &str)] value: T,
	#[implementations(f64, &f64, f32, &f32, u32, &u32, DVec2, &DVec2, &str)] other_value: U,
) -> bool {
	other_value != value
}

/// The less-than operation (<) compares two values and returns true if the first value is less than the second, or false if it is not.
/// If enabled with "Or Equal", the less-than-or-equal operation (<=) will be used instead.
#[node_macro::node(category("Math: Logic"))]
fn less_than<T: core::cmp::PartialOrd<T>>(
	_: impl Ctx,
	#[implementations(f64, &f64, f32, &f32, u32, &u32)] value: T,
	#[implementations(f64, &f64, f32, &f32, u32, &u32)] other_value: T,
	or_equal: bool,
) -> bool {
	if or_equal { value <= other_value } else { value < other_value }
}

/// The greater-than operation (>) compares two values and returns true if the first value is greater than the second, or false if it is not.
/// If enabled with "Or Equal", the greater-than-or-equal operation (>=) will be used instead.
#[node_macro::node(category("Math: Logic"))]
fn greater_than<T: core::cmp::PartialOrd<T>>(
	_: impl Ctx,
	#[implementations(f64, &f64, f32, &f32, u32, &u32)] value: T,
	#[implementations(f64, &f64, f32, &f32, u32, &u32)] other_value: T,
	or_equal: bool,
) -> bool {
	if or_equal { value >= other_value } else { value > other_value }
}

/// The logical or operation (||) returns true if either of the two inputs are true, or false if both are false.
#[node_macro::node(category("Math: Logic"))]
fn logical_or(_: impl Ctx, value: bool, other_value: bool) -> bool {
	value || other_value
}

/// The logical and operation (&&) returns true if both of the two inputs are true, or false if any are false.
#[node_macro::node(category("Math: Logic"))]
fn logical_and(_: impl Ctx, value: bool, other_value: bool) -> bool {
	value && other_value
}

/// The logical not operation (!) reverses true and false value of the input.
#[node_macro::node(category("Math: Logic"))]
fn logical_not(_: impl Ctx, input: bool) -> bool {
	!input
}

#[node_macro::node(category("Math: Logic"))]
async fn switch<T, C: Send + 'n + Clone>(
	#[implementations(Context)] ctx: C,
	condition: bool,
	#[expose]
	#[implementations(
		Context -> String,
		Context -> bool,
		Context -> f64,
		Context -> u32,
		Context -> u64,
		Context -> DVec2,
		Context -> VectorDataTable,
		Context -> DAffine2,
	)]
	if_true: impl Node<C, Output = T>,
	#[expose]
	#[implementations(
		Context -> String,
		Context -> bool,
		Context -> f64,
		Context -> u32,
		Context -> u64,
		Context -> DVec2,
		Context -> VectorDataTable,
		Context -> DAffine2,
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
