use crate::Node;
use crate::list::{Item, List, ListDyn};
use crate::transform::Footprint;
use glam::DVec2;
use std::future::Future;
use std::marker::PhantomData;

// Type
// TODO: Document this
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct TypeNode<N: for<'a> Node<'a, I>, I, O>(pub N, pub PhantomData<(I, O)>);
impl<'i, N, I: 'i, O: 'i> Node<'i, I> for TypeNode<N, I, O>
where
	N: for<'n> Node<'n, I, Output = O>,
{
	type Output = O;
	fn eval(&'i self, input: I) -> Self::Output {
		self.0.eval(input)
	}

	fn reset(&self) {
		self.0.reset();
	}

	fn serialize(&self) -> Option<std::sync::Arc<dyn std::any::Any + Send + Sync>> {
		self.0.serialize()
	}
}
impl<'i, N: for<'a> Node<'a, I>, I: 'i> TypeNode<N, I, <N as Node<'i, I>>::Output> {
	pub fn new(node: N) -> Self {
		Self(node, PhantomData)
	}
}
impl<'i, N: for<'a> Node<'a, I> + Clone, I: 'i> Clone for TypeNode<N, I, <N as Node<'i, I>>::Output> {
	fn clone(&self) -> Self {
		Self(self.0.clone(), self.1)
	}
}
impl<'i, N: for<'a> Node<'a, I> + Copy, I: 'i> Copy for TypeNode<N, I, <N as Node<'i, I>>::Output> {}

/// The [`Convert`] trait allows for conversion between Rust primitive numeric types.
/// Because number casting is lossy, we cannot use the normal [`Into`] trait like we do for other types.
pub trait Convert<T, C>: Sized {
	/// Converts this type into the (usually inferred) output type.
	#[must_use]
	fn convert(self, footprint: Footprint, converter: C) -> impl Future<Output = T> + Send;
}

impl<T: ToString + Send> Convert<String, ()> for T {
	/// Converts this type into a `String` using its `ToString` implementation.
	#[inline]
	async fn convert(self, _: Footprint, _converter: ()) -> String {
		self.to_string()
	}
}

pub trait ListConvert<U> {
	fn convert_item(self) -> U;
}

impl<U, T: ListConvert<U> + Send> Convert<List<U>, ()> for List<T> {
	async fn convert(self, _: Footprint, _: ()) -> List<U> {
		let list: List<U> = self
			.into_iter()
			.map(|row| {
				let (element, attributes) = row.into_parts();
				Item::from_parts(element.convert_item(), attributes)
			})
			.collect();
		list
	}
}

/// Erases a `List<T>`'s element type, exposing only its attributes and row count. Lets nodes that
/// only need attribute access (such as the `read_attribute_*` family) take a single `ListDyn` input
/// instead of monomorphizing over every possible carrier list type.
impl<T: Send> Convert<ListDyn, ()> for List<T> {
	async fn convert(self, _: Footprint, _: ()) -> ListDyn {
		self.into()
	}
}

impl Convert<DVec2, ()> for DVec2 {
	async fn convert(self, _: Footprint, _: ()) -> DVec2 {
		self
	}
}

/// Constructs `Self` from a single anchor point at the given position. Implemented by the vector crate's
/// path type so the `Convert` impl below can build a single-point path without core-types depending on
/// that crate (mirroring how [`ListConvert`] bridges per-item list conversions).
pub trait FromAnchorPosition {
	fn from_anchor_position(position: DVec2) -> Self;
}

// Converts a position's Item wire into a vector path composed of a single anchor point
impl<T: FromAnchorPosition + Send> Convert<List<T>, ()> for Item<DVec2> {
	async fn convert(self, _: Footprint, _: ()) -> List<T> {
		List::new_from_item(Item::new_from_element(T::from_anchor_position(self.into_element())))
	}
}

/// Implements the [`Convert`] trait for conversion between the cartesian product of Rust's primitive numeric types.
macro_rules! impl_convert {
	($from:ty, $to:ty) => {
		impl Convert<$to, ()> for $from {
			async fn convert(self, _: Footprint, _: ()) -> $to {
				self as $to
			}
		}
	};
	($to:ty) => {
		impl_convert!(f32, $to);
		impl_convert!(f64, $to);
		impl_convert!(i8, $to);
		impl_convert!(u8, $to);
		impl_convert!(u16, $to);
		impl_convert!(i16, $to);
		impl_convert!(i32, $to);
		impl_convert!(u32, $to);
		impl_convert!(i64, $to);
		impl_convert!(u64, $to);
		impl_convert!(i128, $to);
		impl_convert!(u128, $to);
		impl_convert!(isize, $to);
		impl_convert!(usize, $to);

		impl Convert<DVec2, ()> for $to {
			async fn convert(self, _: Footprint, _: ()) -> DVec2 {
				DVec2::splat(self as f64)
			}
		}
	};
}
impl_convert!(f32);
impl_convert!(f64);
impl_convert!(i8);
impl_convert!(u8);
impl_convert!(u16);
impl_convert!(i16);
impl_convert!(i32);
impl_convert!(u32);
impl_convert!(i64);
impl_convert!(u64);
impl_convert!(i128);
impl_convert!(u128);
impl_convert!(isize);
impl_convert!(usize);
