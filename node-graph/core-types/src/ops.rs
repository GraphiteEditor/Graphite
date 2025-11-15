use crate::{
	Node,
	table::{Table, TableRow},
	transform::Footprint,
};
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

// trait mentioning inner type in args
pub trait TableConvert<U> {
	fn convert_row(self) -> U;
}

//
impl<U, T: TableConvert<U> + Send> Convert<Table<U>, ()> for Table<T> {
	async fn convert(self, _: Footprint, _: ()) -> Table<U> {
		let table: Table<U> = self
			.into_iter()
			.map(|row| TableRow {
				element: row.element.convert_row(),
				transform: row.transform,
				alpha_blending: row.alpha_blending,
				source_node_id: row.source_node_id,
			})
			.collect();
		table
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
