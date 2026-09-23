use crate::Node;
use crate::math::float_noise::round_away_float_noise;
use crate::transform::Footprint;
use glam::{DAffine2, DVec2};
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

/// Implements the [`Convert`] trait for formatting a type into a `String` via [`ToString`].
macro_rules! impl_convert_to_string {
	($($from:ty),* $(,)?) => {
		$(
			impl Convert<String, ()> for $from {
				#[inline]
				async fn convert(self, _: Footprint, _converter: ()) -> String {
					self.to_string()
				}
			}
		)*
	};
}
impl_convert_to_string!(i64, bool, DVec2, DAffine2);

// Denoised so 0.1 + 0.2 reaches the string as "0.3" rather than "0.30000000000000004"
impl Convert<String, ()> for f64 {
	#[inline]
	async fn convert(self, _: Footprint, _converter: ()) -> String {
		round_away_float_noise(self).to_string()
	}
}

/// Constructs `Self` from a single anchor point at the given position. Implemented by the vector crate's
/// path type so a position wire can convert to a single-point path without core-types depending on that crate.
pub trait FromAnchorPosition {
	fn from_anchor_position(position: DVec2) -> Self;
}

/// Implements the [`Convert`] trait between Rust's primitive numeric types. A float narrowing to an integer rounds to the
/// nearest whole number (half away from zero), the graph's one Number to Integer rule, so float noise like 5.999999 reaches
/// an integer connector as 6 rather than truncating to 5. Every other pair is a plain `as` cast.
macro_rules! impl_convert {
	($from:ty => $to:ty) => {
		impl Convert<$to, ()> for $from {
			async fn convert(self, _: Footprint, _: ()) -> $to {
				self as $to
			}
		}
	};
	($from:ty => round $to:ty) => {
		impl Convert<$to, ()> for $from {
			async fn convert(self, _: Footprint, _: ()) -> $to {
				self.round() as $to
			}
		}
	};
	(from_integers $to:ty) => {
		impl_convert!(i64 => $to);

		impl Convert<DVec2, ()> for $to {
			async fn convert(self, _: Footprint, _: ()) -> DVec2 {
				DVec2::splat(self as f64)
			}
		}
	};
	(float $to:ty) => {
		impl_convert!(f64 => $to);
		impl_convert!(from_integers $to);
	};
	(integer $to:ty) => {
		impl_convert!(f64 => round $to);
		impl_convert!(from_integers $to);
	};
}
impl_convert!(float f64);
impl_convert!(integer i64);

/// Implements the [`Convert`] trait from `bool` into each numeric type, embedding `false` and `true` as exactly 0 and 1.
/// The reverse direction is deliberately absent: a number only becomes a truth value through an explicit comparison.
macro_rules! impl_convert_from_bool {
	($($to:ty),* $(,)?) => {
		$(
			impl Convert<$to, ()> for bool {
				async fn convert(self, _: Footprint, _: ()) -> $to {
					self as u8 as $to
				}
			}
		)*
	};
}
impl_convert_from_bool!(f64, i64,);

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn a_float_narrows_to_the_nearest_integer() {
		let footprint = Footprint::default();
		assert_eq!(Convert::<i64, ()>::convert(5.999_999_f64, footprint, ()).await, 6, "float noise rounds up to the whole number it meant");
		assert_eq!(Convert::<i64, ()>::convert(2.5_f64, footprint, ()).await, 3, "ties round away from zero");
		assert_eq!(Convert::<i64, ()>::convert(-1.5_f64, footprint, ()).await, -2);
		assert_eq!(Convert::<f64, ()>::convert(7_i64, footprint, ()).await, 7., "an integer widens exactly");
	}
}
