use std::ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign};

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Polynomial<const N: usize> {
	coeffs: [f64; N],
}

impl<const N: usize> Polynomial<N> {
	pub fn new(value: [f64; N]) -> Polynomial<N> {
		Polynomial { coeffs: value }
	}

	pub fn zero() -> Polynomial<N> {
		Polynomial { coeffs: [0.; N] }
	}

	pub fn coeffs(&self) -> &[f64; N] {
		&self.coeffs
	}

	pub fn coeffs_mut(&mut self) -> &mut [f64; N] {
		&mut self.coeffs
	}

	pub fn eval(&self, value: f64) -> f64 {
		self.coeffs.iter().rev().copied().reduce(|acc, x| acc * value + x).unwrap()
	}

	pub fn as_size<const M: usize>(&self) -> Option<Polynomial<M>> {
		let mut coeffs = [0.; M];

		if M >= N {
			coeffs[..N].copy_from_slice(&self.coeffs);
		} else if self.coeffs.iter().rev().take(N - M).all(|&x| x == 0.) {
			coeffs.copy_from_slice(&self.coeffs[..M])
		} else {
			return None;
		}

		Some(Polynomial { coeffs })
	}

	pub fn derivative_mut(&mut self) {
		self.coeffs.iter_mut().enumerate().for_each(|(index, x)| *x *= index as f64);
		self.coeffs.rotate_left(1);
	}

	pub fn antiderivative_mut(&mut self) -> Option<()> {
		if self.coeffs[N - 1] != 0. {
			return None;
		}
		self.coeffs.rotate_right(1);
		self.coeffs.iter_mut().enumerate().skip(1).for_each(|(index, x)| *x /= index as f64);
		Some(())
	}

	pub fn derivative(&self) -> Polynomial<N> {
		let mut ans = *self;
		ans.derivative_mut();
		ans
	}

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

impl<const N: usize> AddAssign<&Polynomial<N>> for Polynomial<N> {
	fn add_assign(&mut self, rhs: &Polynomial<N>) {
		self.coeffs.iter_mut().zip(rhs.coeffs.iter()).for_each(|(a, b)| *a += b);
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
		output.coeffs.iter_mut().for_each(|x| *x = -*x);
		output
	}
}

impl<const N: usize> Neg for Polynomial<N> {
	type Output = Polynomial<N>;

	fn neg(mut self) -> Polynomial<N> {
		self.coeffs.iter_mut().for_each(|x| *x = -*x);
		self
	}
}

impl<const N: usize> SubAssign<&Polynomial<N>> for Polynomial<N> {
	fn sub_assign(&mut self, rhs: &Polynomial<N>) {
		self.coeffs.iter_mut().zip(rhs.coeffs.iter()).for_each(|(a, b)| *a -= b);
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
			self.coeffs[i] = self.coeffs[i] * rhs.coeffs[0];
			for j in 0..i {
				self.coeffs[i] += self.coeffs[j] * rhs.coeffs[i - j];
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

		p.coeffs_mut()[0] = 0.;
		assert_eq!(p_deriv.antiderivative().unwrap(), p);

		assert_eq!(p.antiderivative(), None);
	}
}
