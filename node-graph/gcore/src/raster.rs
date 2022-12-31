use crate::Node;

pub mod color;
pub use self::color::Color;

#[derive(Debug, Clone, Copy, Default)]
pub struct GrayscaleColorNode;

impl Node<Color> for GrayscaleColorNode {
	type Output = Color;
	fn eval(self, color: Color) -> Color {
		let avg = (color.r() + color.g() + color.b()) / 3.0;
		Color::from_rgbaf32_unchecked(avg, avg, avg, color.a())
	}
}
impl<'n> Node<Color> for &'n GrayscaleColorNode {
	type Output = Color;
	fn eval(self, color: Color) -> Color {
		let avg = (color.r() + color.g() + color.b()) / 3.0;
		Color::from_rgbaf32_unchecked(avg, avg, avg, color.a())
	}
}

impl GrayscaleColorNode {
	pub fn new() -> Self {
		Self
	}
}

#[derive(Debug, Clone, Copy)]
pub struct BrightenColorNode<N: Node<(), Output = f32>>(N);

impl<N: Node<(), Output = f32>> Node<Color> for BrightenColorNode<N> {
	type Output = Color;
	fn eval(self, color: Color) -> Color {
		let brightness = self.0.eval(());
		let per_channel = |col: f32| (col + brightness / 255.).clamp(0., 1.);
		Color::from_rgbaf32_unchecked(per_channel(color.r()), per_channel(color.g()), per_channel(color.b()), color.a())
	}
}
impl<N: Node<(), Output = f32> + Copy> Node<Color> for &BrightenColorNode<N> {
	type Output = Color;
	fn eval(self, color: Color) -> Color {
		let brightness = self.0.eval(());
		let per_channel = |col: f32| (col + brightness / 255.).clamp(0., 1.);
		Color::from_rgbaf32_unchecked(per_channel(color.r()), per_channel(color.g()), per_channel(color.b()), color.a())
	}
}

impl<N: Node<(), Output = f32> + Copy> BrightenColorNode<N> {
	pub fn new(node: N) -> Self {
		Self(node)
	}
}

#[derive(Debug, Clone, Copy)]
pub struct GammaColorNode<N: Node<(), Output = f32>>(N);

impl<N: Node<(), Output = f32>> Node<Color> for GammaColorNode<N> {
	type Output = Color;
	fn eval(self, color: Color) -> Color {
		let gamma = self.0.eval(());
		let per_channel = |col: f32| col.powf(gamma);
		Color::from_rgbaf32_unchecked(per_channel(color.r()), per_channel(color.g()), per_channel(color.b()), color.a())
	}
}
impl<N: Node<(), Output = f32> + Copy> Node<Color> for &GammaColorNode<N> {
	type Output = Color;
	fn eval(self, color: Color) -> Color {
		let gamma = self.0.eval(());
		let per_channel = |col: f32| col.powf(gamma);
		Color::from_rgbaf32_unchecked(per_channel(color.r()), per_channel(color.g()), per_channel(color.b()), color.a())
	}
}

impl<N: Node<(), Output = f32> + Copy> GammaColorNode<N> {
	pub fn new(node: N) -> Self {
		Self(node)
	}
}

#[derive(Debug, Clone, Copy)]
#[cfg(not(target_arch = "spirv"))]
pub struct HueShiftColorNode<N: Node<(), Output = f32>>(N);

#[cfg(not(target_arch = "spirv"))]
impl<N: Node<(), Output = f32>> Node<Color> for HueShiftColorNode<N> {
	type Output = Color;
	fn eval(self, color: Color) -> Color {
		let hue_shift = self.0.eval(());
		let [hue, saturation, lightness, alpha] = color.to_hsla();
		Color::from_hsla(hue + hue_shift / 360., saturation, lightness, alpha)
	}
}
#[cfg(not(target_arch = "spirv"))]
impl<N: Node<(), Output = f32> + Copy> Node<Color> for &HueShiftColorNode<N> {
	type Output = Color;
	fn eval(self, color: Color) -> Color {
		let hue_shift = self.0.eval(());
		let [hue, saturation, lightness, alpha] = color.to_hsla();
		Color::from_hsla(hue + hue_shift / 360., saturation, lightness, alpha)
	}
}

#[cfg(not(target_arch = "spirv"))]
impl<N: Node<(), Output = f32> + Copy> HueShiftColorNode<N> {
	pub fn new(node: N) -> Self {
		Self(node)
	}
}

pub struct ForEachNode<MN>(pub MN);

impl<'n, I: Iterator<Item = S>, MN: 'n, S> Node<I> for &'n ForEachNode<MN>
where
	&'n MN: Node<S, Output = ()>,
{
	type Output = ();
	fn eval(self, input: I) -> Self::Output {
		input.for_each(|x| (&self.0).eval(x))
	}
}

#[cfg(feature = "alloc")]
pub use image::Image;
#[cfg(feature = "alloc")]
mod image {
	use super::Color;
	use alloc::vec::Vec;
	use dyn_any::{DynAny, StaticType};
	#[derive(Clone, Debug, PartialEq, DynAny, Default)]
	#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
	pub struct Image {
		pub width: u32,
		pub height: u32,
		pub data: Vec<Color>,
	}

	impl Image {
		pub const fn empty() -> Self {
			Self {
				width: 0,
				height: 0,
				data: Vec::new(),
			}
		}
	}

	impl IntoIterator for Image {
		type Item = Color;
		type IntoIter = alloc::vec::IntoIter<Color>;
		fn into_iter(self) -> Self::IntoIter {
			self.data.into_iter()
		}
	}

	impl<'a> IntoIterator for &'a Image {
		type Item = &'a Color;
		type IntoIter = alloc::slice::Iter<'a, Color>;
		fn into_iter(self) -> Self::IntoIter {
			self.data.iter()
		}
	}
}

/*pub struct MutWrapper<N>(pub N);

impl<'n, T: Clone, N> Node<&'n mut T> for &'n MutWrapper<N>
where
	&'n N: Node<T, Output = T>,
{
	type Output = ();
	fn eval(self, value: &'n mut T) {
		*value = (&self.0).eval(value.clone());
	}
}*/

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn map_node() {
		// let array = &mut [Color::from_rgbaf32(1.0, 0.0, 0.0, 1.0).unwrap()];
		(&GrayscaleColorNode).eval(Color::from_rgbf32_unchecked(1., 0., 0.));
		/*let map = ForEachNode(MutWrapper(GrayscaleNode));
		(&map).eval(array.iter_mut());
		assert_eq!(array[0], Color::from_rgbaf32(0.33333334, 0.33333334, 0.33333334, 1.0).unwrap());*/
	}
}
