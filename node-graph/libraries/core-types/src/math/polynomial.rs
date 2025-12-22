use kurbo::PathSeg;
use std::fmt::{self, Display, Formatter};
use std::ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// A struct that represents a polynomial with a maximum degree of `N-1`.
///
/// It provides basic mathematical operations for polynomials like addition, multiplication, differentiation, integration, etc.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Polynomial<const N: usize> {
	coefficients: [f64; N],
}

impl<const N: usize> Polynomial<N> {
	/// Create a new polynomial from the coefficients given in the array.
	///
	/// The coefficient for nth degree is at the nth index in array. Therefore the order of coefficients are reversed than the usual order for writing polynomials mathematically.
	pub fn new(coefficients: [f64; N]) -> Polynomial<N> {
		Polynomial { coefficients }
	}

	/// Create a polynomial where all its coefficients are zero.
	pub fn zero() -> Polynomial<N> {
		Polynomial { coefficients: [0.; N] }
	}

	/// Return an immutable reference to the coefficients.
	///
	/// The coefficient for nth degree is at the nth index in array. Therefore the order of coefficients are reversed than the usual order for writing polynomials mathematically.
	pub fn coefficients(&self) -> &[f64; N] {
		&self.coefficients
	}

	/// Return a mutable reference to the coefficients.
	///
	/// The coefficient for nth degree is at the nth index in array. Therefore the order of coefficients are reversed than the usual order for writing polynomials mathematically.
	pub fn coefficients_mut(&mut self) -> &mut [f64; N] {
		&mut self.coefficients
	}

	/// Evaluate the polynomial at `value`.
	pub fn eval(&self, value: f64) -> f64 {
		self.coefficients.iter().rev().copied().reduce(|acc, x| acc * value + x).unwrap()
	}

	/// Return the same polynomial but with a different maximum degree of `M-1`.\
	///
	/// Returns `None` if the polynomial cannot fit in the specified size.
	pub fn as_size<const M: usize>(&self) -> Option<Polynomial<M>> {
		let mut coefficients = [0.; M];

		if M >= N {
			coefficients[..N].copy_from_slice(&self.coefficients);
		} else if self.coefficients.iter().rev().take(N - M).all(|&x| x == 0.) {
			coefficients.copy_from_slice(&self.coefficients[..M])
		} else {
			return None;
		}

		Some(Polynomial { coefficients })
	}

	/// Computes the derivative in place.
	pub fn derivative_mut(&mut self) {
		self.coefficients.iter_mut().enumerate().for_each(|(index, x)| *x *= index as f64);
		self.coefficients.rotate_left(1);
	}

	/// Computes the antiderivative at `C = 0` in place.
	///
	/// Returns `None` if the polynomial is not big enough to accommodate the extra degree.
	pub fn antiderivative_mut(&mut self) -> Option<()> {
		if self.coefficients[N - 1] != 0. {
			return None;
		}
		self.coefficients.rotate_right(1);
		self.coefficients.iter_mut().enumerate().skip(1).for_each(|(index, x)| *x /= index as f64);
		Some(())
	}

	/// Computes the polynomial's derivative.
	pub fn derivative(&self) -> Polynomial<N> {
		let mut ans = *self;
		ans.derivative_mut();
		ans
	}

	/// Computes the antiderivative at `C = 0`.
	///
	/// Returns `None` if the polynomial is not big enough to accommodate the extra degree.
	pub fn antiderivative(&self) -> Option<Polynomial<N>> {
		let mut ans = *self;
		ans.antiderivative_mut()?;
		Some(ans)
	}
}

impl<const N: usize> Default for Polynomial<N> {
	fn default() -> Self {
		Self::zero()
	}
}

impl<const N: usize> Display for Polynomial<N> {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		let mut first = true;
		for (index, coefficient) in self.coefficients.iter().enumerate().rev().filter(|&(_, &coefficient)| coefficient != 0.) {
			if first {
				first = false;
			} else {
				f.write_str(" + ")?
			}

			coefficient.fmt(f)?;
			if index == 0 {
				continue;
			}
			f.write_str("x")?;
			if index == 1 {
				continue;
			}
			f.write_str("^")?;
			index.fmt(f)?;
		}

		Ok(())
	}
}

impl<const N: usize> AddAssign<&Polynomial<N>> for Polynomial<N> {
	fn add_assign(&mut self, rhs: &Polynomial<N>) {
		self.coefficients.iter_mut().zip(rhs.coefficients.iter()).for_each(|(a, b)| *a += b);
	}
}

impl<const N: usize> Add for &Polynomial<N> {
	type Output = Polynomial<N>;

	fn add(self, other: &Polynomial<N>) -> Polynomial<N> {
		let mut output = *self;
		output += other;
		output
	}
}

impl<const N: usize> Neg for &Polynomial<N> {
	type Output = Polynomial<N>;

	fn neg(self) -> Polynomial<N> {
		let mut output = *self;
		output.coefficients.iter_mut().for_each(|x| *x = -*x);
		output
	}
}

impl<const N: usize> Neg for Polynomial<N> {
	type Output = Polynomial<N>;

	fn neg(mut self) -> Polynomial<N> {
		self.coefficients.iter_mut().for_each(|x| *x = -*x);
		self
	}
}

impl<const N: usize> SubAssign<&Polynomial<N>> for Polynomial<N> {
	fn sub_assign(&mut self, rhs: &Polynomial<N>) {
		self.coefficients.iter_mut().zip(rhs.coefficients.iter()).for_each(|(a, b)| *a -= b);
	}
}

impl<const N: usize> Sub for &Polynomial<N> {
	type Output = Polynomial<N>;

	fn sub(self, other: &Polynomial<N>) -> Polynomial<N> {
		let mut output = *self;
		output -= other;
		output
	}
}

impl<const N: usize> MulAssign<&Polynomial<N>> for Polynomial<N> {
	fn mul_assign(&mut self, rhs: &Polynomial<N>) {
		for i in (0..N).rev() {
			self.coefficients[i] = self.coefficients[i] * rhs.coefficients[0];
			for j in 0..i {
				self.coefficients[i] += self.coefficients[j] * rhs.coefficients[i - j];
			}
		}
	}
}

impl<const N: usize> Mul for &Polynomial<N> {
	type Output = Polynomial<N>;

	fn mul(self, other: &Polynomial<N>) -> Polynomial<N> {
		let mut output = *self;
		output *= other;
		output
	}
}

/// Returns two [`Polynomial`]s representing the parametric equations for x and y coordinates of the bezier curve respectively.
/// The domain of both the equations are from t=0.0 representing the start and t=1.0 representing the end of the bezier curve.
pub fn pathseg_to_parametric_polynomial(segment: PathSeg) -> (Polynomial<4>, Polynomial<4>) {
	match segment {
		PathSeg::Line(line) => {
			let term1 = line.p0 - line.p1;
			(Polynomial::new([line.p0.x, term1.x, 0., 0.]), Polynomial::new([line.p0.y, term1.y, 0., 0.]))
		}
		PathSeg::Quad(quad_bez) => {
			let term1 = 2. * (quad_bez.p1 - quad_bez.p0);
			let term2 = quad_bez.p0 - 2. * quad_bez.p1.to_vec2() + quad_bez.p2.to_vec2();

			(Polynomial::new([quad_bez.p0.x, term1.x, term2.x, 0.]), Polynomial::new([quad_bez.p0.y, term1.y, term2.y, 0.]))
		}
		PathSeg::Cubic(cubic_bez) => {
			let term1 = 3. * (cubic_bez.p1 - cubic_bez.p0);
			let term2 = 3. * (cubic_bez.p2 - cubic_bez.p1) - term1;
			let term3 = cubic_bez.p3 - cubic_bez.p0 - term2 - term1;

			(
				Polynomial::new([cubic_bez.p0.x, term1.x, term2.x, term3.x]),
				Polynomial::new([cubic_bez.p0.y, term1.y, term2.y, term3.y]),
			)
		}
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn evaluation() {
		let p = Polynomial::new([1., 2., 3.]);

		assert_eq!(p.eval(1.), 6.);
		assert_eq!(p.eval(2.), 17.);
	}

	#[test]
	fn size_change() {
		let p1 = Polynomial::new([1., 2., 3.]);
		let p2 = Polynomial::new([1., 2., 3., 0.]);

		assert_eq!(p1.as_size(), Some(p2));
		assert_eq!(p2.as_size(), Some(p1));

		assert_eq!(p2.as_size::<2>(), None);
	}

	#[test]
	fn addition_and_subtaction() {
		let p1 = Polynomial::new([1., 2., 3.]);
		let p2 = Polynomial::new([4., 5., 6.]);

		let addition = Polynomial::new([5., 7., 9.]);
		let subtraction = Polynomial::new([-3., -3., -3.]);

		assert_eq!(&p1 + &p2, addition);
		assert_eq!(&p1 - &p2, subtraction);
	}

	#[test]
	fn multiplication() {
		let p1 = Polynomial::new([1., 2., 3.]).as_size().unwrap();
		let p2 = Polynomial::new([4., 5., 6.]).as_size().unwrap();

		let multiplication = Polynomial::new([4., 13., 28., 27., 18.]);

		assert_eq!(&p1 * &p2, multiplication);
	}

	#[test]
	fn derivative_and_antiderivative() {
		let mut p = Polynomial::new([1., 2., 3.]);
		let p_deriv = Polynomial::new([2., 6., 0.]);

		assert_eq!(p.derivative(), p_deriv);

		p.coefficients_mut()[0] = 0.;
		assert_eq!(p_deriv.antiderivative().unwrap(), p);

		assert_eq!(p.antiderivative(), None);
	}

	#[test]
	fn display() {
		let p = Polynomial::new([1., 2., 0., 3.]);

		assert_eq!(format!("{p:.2}"), "3.00x^3 + 2.00x + 1.00");
	}
}
