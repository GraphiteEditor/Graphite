#![allow(non_snake_case)]
#![allow(clippy::too_many_arguments)]

use core::f64;

const CUBIC_RESCAL_FACT: f64 = 3.488062113727083E+102; //= pow(DBL_MAX,1.0/3.0)/1.618034;
const QUART_RESCAL_FACT: f64 = 7.156344627944542E+76; // = pow(DBL_MAX,1.0/4.0)/1.618034;
const MACHEPS: f64 = 2.2204460492503131E-16; // DBL_EPSILON

const M_PI: f64 = std::f64::consts::PI;

#[derive(Copy, Clone, Debug, Default)]
struct Complex(f64, f64);
impl std::ops::Add for Complex {
	type Output = Self;
	fn add(self, rhs: Self) -> Self {
		Complex(self.0 + rhs.0, self.1 + rhs.1)
	}
}
impl std::ops::Sub for Complex {
	type Output = Self;
	fn sub(self, rhs: Self) -> Self {
		Complex(self.0 - rhs.0, self.1 - rhs.1)
	}
}
impl std::ops::Mul for Complex {
	type Output = Self;
	fn mul(self, rhs: Self) -> Self {
		Complex(self.0 * rhs.0 - self.1 * rhs.1, self.0 * rhs.1 + self.1 * rhs.0)
	}
}
impl std::ops::Div for Complex {
	type Output = Self;
	fn div(self, rhs: Self) -> Self {
		let d = rhs.0 * rhs.0 + rhs.1 * rhs.1;
		Complex((self.0 * rhs.0 + self.1 * rhs.1) / d, (self.1 * rhs.0 - self.0 * rhs.1) / d)
	}
}
impl std::ops::Neg for Complex {
	type Output = Self;
	fn neg(self) -> Self {
		Complex(-self.0, -self.1)
	}
}
impl Complex {
	fn new(real: f64, imag: f64) -> Self {
		Complex(real, imag)
	}

	fn real(real: f64) -> Self {
		Complex(real, 0.0)
	}

	fn imag(imag: f64) -> Self {
		Complex(0.0, imag)
	}

	fn conj(self) -> Self {
		Complex(self.0, -self.1)
	}
}

fn fabs(x: f64) -> f64 {
	x.abs()
}
fn copysign(x: f64, y: f64) -> f64 {
	x.copysign(y)
}
fn sqrt(x: f64) -> f64 {
	x.sqrt()
}
fn oqs_max2(a: f64, b: f64) -> f64 {
	if a >= b {
		a
	} else {
		b
	}
}
fn oqs_max3(a: f64, b: f64, c: f64) -> f64 {
	let t = oqs_max2(a, b);
	oqs_max2(t, c)
}
fn acos(x: f64) -> f64 {
	x.acos()
}
fn cos(x: f64) -> f64 {
	x.cos()
}
fn cbrt(x: f64) -> f64 {
	x.cbrt()
}
fn pow(x: f64, y: f64) -> f64 {
	x.powf(y)
}
fn cabs(x: Complex) -> f64 {
	(x.0 * x.0 + x.1 * x.1).sqrt()
}
fn csqrt(x: Complex) -> Complex {
	let r = (x.0 * x.0 + x.1 * x.1).sqrt();
	let t = 0.5 * (x.1 / x.0).atan();
	Complex(r.cos(), r.sin()) * Complex(t.cos(), t.sin())
}

fn oqs_solve_cubic_analytic_depressed_handle_inf(b: f64, c: f64) -> f64 {
	/* find analytically the dominant root of a depressed cubic x^3+b*x+c
	 * where coefficients b and c are large (see sec. 2.2 in the manuscript) */

	const PI2: f64 = M_PI / 2.0;
	const TWOPI: f64 = 2.0 * M_PI;

	let Q = -b / 3.0;
	let R = 0.5 * c;

	if R == 0. {
		return if b <= 0. { sqrt(-b) } else { 0. };
	}

	let KK = if fabs(Q) < fabs(R) {
		let QR = Q / R;
		let QRSQ = QR * QR;
		1.0 - Q * QRSQ
	} else {
		let RQ = R / Q;
		copysign(1.0, Q) * (RQ * RQ / Q - 1.0)
	};

	if KK < 0.0 {
		let sqrtQ = sqrt(Q);
		let theta = acos((R / fabs(Q)) / sqrtQ);
		if theta < PI2 {
			-2.0 * sqrtQ * cos(theta / 3.0)
		} else {
			-2.0 * sqrtQ * cos((theta + TWOPI) / 3.0)
		}
	} else {
		let A = if fabs(Q) < fabs(R) {
			-copysign(1.0, R) * cbrt(fabs(R) * (1.0 + sqrt(KK)))
		} else {
			-copysign(1.0, R) * cbrt(fabs(R) + sqrt(fabs(Q)) * fabs(Q) * sqrt(KK))
		};
		let B = if A == 0.0 { 0.0 } else { Q / A };
		A + B
	}
}

fn oqs_solve_cubic_analytic_depressed(b: f64, c: f64) -> f64 {
	/* find analytically the dominant root of a depressed cubic x^3+b*x+c
	 * (see sec. 2.2 in the manuscript) */

	let Q = -b / 3.0;
	let R = 0.5 * c;
	if fabs(Q) > 1E102 || fabs(R) > 1E154 {
		return oqs_solve_cubic_analytic_depressed_handle_inf(b, c);
	}

	let Q3 = Q * Q * Q;
	let R2 = R * R;
	if R2 < Q3 {
		let theta = acos(R / sqrt(Q3));
		let sqrtQ = -2.0 * sqrt(Q);
		if theta < M_PI / 2. {
			sqrtQ * cos(theta / 3.0)
		} else {
			sqrtQ * cos((theta + 2.0 * M_PI) / 3.0)
		}
	} else {
		let A = -copysign(1.0, R) * pow(fabs(R) + sqrt(R2 - Q3), 1.0 / 3.0);
		let B = if A == 0.0 { 0.0 } else { Q / A };
		A + B /* this is always largest root even if A=B */
	}
}

fn oqs_calc_phi0(a: f64, b: f64, c: f64, d: f64, scaled: bool) -> f64 {
	/* find phi0 as the dominant root of the depressed and shifted cubic
	 * in eq. (79) (see also the discussion in sec. 2.2 of the manuscript) */
	let mut diskr = 9. * a * a - 24. * b;
	/* eq. (87) */
	let s = if diskr > 0.0 {
		diskr = sqrt(diskr);
		if a > 0.0 {
			-2. * b / (3. * a + diskr)
		} else {
			-2. * b / (3. * a - diskr)
		}
	} else {
		-a / 4.
	};
	/* eqs. (83) */
	let aq = a + 4. * s;
	let bq = b + 3. * s * (a + 2. * s);
	let cq = c + s * (2. * b + s * (3. * a + 4. * s));
	let dq = d + s * (c + s * (b + s * (a + s)));
	let gg = bq * bq / 9.;
	let hh = aq * cq;

	let mut g = hh - 4. * dq - 3. * gg; /* eq. (85) */
	let mut h = (8. * dq + hh - 2. * gg) * bq / 3. - cq * cq - dq * aq * aq; /* eq. (86) */

	let mut rmax = oqs_solve_cubic_analytic_depressed(g, h);
	if rmax.is_nan() || rmax.is_infinite() {
		rmax = oqs_solve_cubic_analytic_depressed_handle_inf(g, h);
		if (rmax.is_nan() || rmax.is_infinite()) && scaled {
			// try harder: rescale also the depressed cubic if quartic has been already rescaled
			let rfact = CUBIC_RESCAL_FACT;
			let rfactsq = rfact * rfact;
			// let ggss = gg / rfactsq;
			// let hhss = hh / rfactsq;
			let dqss = dq / rfactsq;
			let aqs = aq / rfact;
			let bqs = bq / rfact;
			let cqs = cq / rfact;
			let ggss = bqs * bqs / 9.0;
			let hhss = aqs * cqs;
			g = hhss - 4.0 * dqss - 3.0 * ggss;
			h = (8.0 * dqss + hhss - 2.0 * ggss) * bqs / 3. - cqs * (cqs / rfact) - (dq / rfact) * aqs * aqs;
			rmax = oqs_solve_cubic_analytic_depressed(g, h);
			rmax = if rmax.is_nan() || rmax.is_infinite() {
				oqs_solve_cubic_analytic_depressed_handle_inf(g, h)
			} else {
				rmax
			};
			rmax *= rfact;
		}
	}

	/* Newton-Raphson used to refine phi0 (see end of sec. 2.2 in the manuscript) */
	let mut x = rmax;
	let xsq = x * x;
	let xxx = x * xsq;
	let gx = g * x;
	let f = x * (xsq + g) + h;
	let maxtt = if fabs(xxx) > fabs(gx) { fabs(xxx) } else { fabs(gx) };
	let maxtt = if fabs(h) > maxtt { fabs(h) } else { maxtt };

	if fabs(f) > MACHEPS * maxtt {
		// for (iter=0; iter < 8; iter++) {
		for _ in 0..8 {
			let df = 3.0 * xsq + g;
			if df == 0. {
				break;
			}
			let xold = x;
			x += -f / df;
			let fold = f;
			let xsq = x * x;
			let f = x * (xsq + g) + h;
			if f == 0. {
				break;
			}

			if fabs(f) >= fabs(fold) {
				x = xold;
				break;
			}
		}
	}
	x
}

fn oqs_calc_err_ldlt(b: f64, c: f64, d: f64, d2: f64, l1: f64, l2: f64, l3: f64) -> f64 {
	/* Eqs. (29) and (30) in the manuscript */
	let mut sum = if b == 0. { fabs(d2 + l1 * l1 + 2.0 * l3) } else { fabs(((d2 + l1 * l1 + 2.0 * l3) - b) / b) };
	sum += if c == 0. {
		fabs(2.0 * d2 * l2 + 2.0 * l1 * l3)
	} else {
		fabs(((2.0 * d2 * l2 + 2.0 * l1 * l3) - c) / c)
	};
	sum += if d == 0. { fabs(d2 * l2 * l2 + l3 * l3) } else { fabs(((d2 * l2 * l2 + l3 * l3) - d) / d) };
	sum
}

fn oqs_calc_err_abcd_cmplx(a: f64, b: f64, c: f64, d: f64, aq: Complex, bq: Complex, cq: Complex, dq: Complex) -> f64 {
	/* Eqs. (68) and (69) in the manuscript for complex alpha1 (aq), beta1 (bq), alpha2 (cq) and beta2 (dq) */
	let mut sum = if d == 0. { cabs(bq * dq) } else { cabs((bq * dq - Complex::real(d)) / Complex::real(d)) };
	sum += if c == 0. {
		cabs(bq * cq + aq * dq)
	} else {
		cabs(((bq * cq + aq * dq) - Complex::real(c)) / Complex::real(c))
	};
	sum += if b == 0. {
		cabs(bq + aq * cq + dq)
	} else {
		cabs(((bq + aq * cq + dq) - Complex::real(b)) / Complex::real(b))
	};
	sum += if a == 0. { cabs(aq + cq) } else { cabs(((aq + cq) - Complex::real(a)) / Complex::real(a)) };
	sum
}

fn oqs_calc_err_abcd(a: f64, b: f64, c: f64, d: f64, aq: f64, bq: f64, cq: f64, dq: f64) -> f64 {
	/* Eqs. (68) and (69) in the manuscript for real alpha1 (aq), beta1 (bq), alpha2 (cq) and beta2 (dq)*/
	let mut sum = if d == 0. { fabs(bq * dq) } else { fabs((bq * dq - d) / d) };
	sum += if c == 0. { fabs(bq * cq + aq * dq) } else { fabs(((bq * cq + aq * dq) - c) / c) };
	sum += if b == 0. { fabs(bq + aq * cq + dq) } else { fabs(((bq + aq * cq + dq) - b) / b) };
	sum += if a == 0. { fabs(aq + cq) } else { fabs(((aq + cq) - a) / a) };
	sum
}

fn oqs_calc_err_abc(a: f64, b: f64, c: f64, aq: f64, bq: f64, cq: f64, dq: f64) -> f64 {
	/* Eqs. (48)-(51) in the manuscript */
	let mut sum = if c == 0. { fabs(bq * cq + aq * dq) } else { fabs(((bq * cq + aq * dq) - c) / c) };
	sum += if b == 0. { fabs(bq + aq * cq + dq) } else { fabs(((bq + aq * cq + dq) - b) / b) };
	sum += if a == 0. { fabs(aq + cq) } else { fabs(((aq + cq) - a) / a) };
	sum
}

fn oqs_NRabcd(a: f64, b: f64, c: f64, d: f64, AQ: &mut f64, BQ: &mut f64, CQ: &mut f64, DQ: &mut f64) {
	/* Newton-Raphson described in sec. 2.3 of the manuscript for complex
	 * coefficients a,b,c,d */
	let mut xold = [0.; 4];
	let mut dx = [0.; 4];
	let mut Jinv = [[0.; 4]; 4];

	let mut x = [*AQ, *BQ, *CQ, *DQ];
	let vr = [d, c, b, a];
	let mut fvec = [x[1] * x[3] - d, x[1] * x[2] + x[0] * x[3] - c, x[1] + x[0] * x[2] + x[3] - b, x[0] + x[2] - a];
	let mut errf = 0.;
	for k1 in 0..4 {
		errf += if vr[k1] == 0. { fabs(fvec[k1]) } else { fabs(fvec[k1] / vr[k1]) };
	}
	for _ in 0..8 {
		let x02 = x[0] - x[2];
		let det = x[1] * x[1] + x[1] * (-x[2] * x02 - 2.0 * x[3]) + x[3] * (x[0] * x02 + x[3]);
		if det == 0.0 {
			break;
		}
		Jinv[0][0] = x02;
		Jinv[0][1] = x[3] - x[1];
		Jinv[0][2] = x[1] * x[2] - x[0] * x[3];
		Jinv[0][3] = -x[1] * Jinv[0][1] - x[0] * Jinv[0][2];
		Jinv[1][0] = x[0] * Jinv[0][0] + Jinv[0][1];
		Jinv[1][1] = -x[1] * Jinv[0][0];
		Jinv[1][2] = -x[1] * Jinv[0][1];
		Jinv[1][3] = -x[1] * Jinv[0][2];
		Jinv[2][0] = -Jinv[0][0];
		Jinv[2][1] = -Jinv[0][1];
		Jinv[2][2] = -Jinv[0][2];
		Jinv[2][3] = Jinv[0][2] * x[2] + Jinv[0][1] * x[3];
		Jinv[3][0] = -x[2] * Jinv[0][0] - Jinv[0][1];
		Jinv[3][1] = Jinv[0][0] * x[3];
		Jinv[3][2] = x[3] * Jinv[0][1];
		Jinv[3][3] = x[3] * Jinv[0][2];

		for k1 in 0..4 {
			dx[k1] = 0.;
			for k2 in 0..4 {
				dx[k1] += Jinv[k1][k2] * fvec[k2];
			}
		}
		for k1 in 0..4 {
			xold[k1] = x[k1];
		}

		for k1 in 0..4 {
			x[k1] += -dx[k1] / det;
		}
		fvec[0] = x[1] * x[3] - d;
		fvec[1] = x[1] * x[2] + x[0] * x[3] - c;
		fvec[2] = x[1] + x[0] * x[2] + x[3] - b;
		fvec[3] = x[0] + x[2] - a;
		let errfold = errf;
		errf = 0.;
		for k1 in 0..4 {
			errf += if vr[k1] == 0. { fabs(fvec[k1]) } else { fabs(fvec[k1] / vr[k1]) };
		}
		if errf == 0. {
			break;
		}
		if errf >= errfold {
			for k1 in 0..4 {
				x[k1] = xold[k1];
			}
			break;
		}
	}
	*AQ = x[0];
	*BQ = x[1];
	*CQ = x[2];
	*DQ = x[3];
}

fn oqs_solve_quadratic(a: f64, b: f64) -> [Complex; 2] {
	let diskr = a * a - 4. * b;
	if diskr >= 0.0 {
		let div = if a >= 0.0 { -a - sqrt(diskr) } else { -a + sqrt(diskr) };

		let zmax = div / 2.;

		let zmin = if zmax == 0.0 { 0.0 } else { b / zmax };

		[Complex::real(zmax), Complex::real(zmin)]
	} else {
		let sqrtd = sqrt(-diskr);
		[Complex::new(-a / 2., sqrtd / 2.), Complex::new(-a / 2., -sqrtd / 2.)]
	}
}

pub fn oqs_quartic_solver(coeff: [f64; 5]) -> [f64; 4] {
	/* USAGE:
	 *
	 * This routine calculates the roots of the quartic equation
	 *
	 * coeff[4]*x^4 + coeff[3]*x^3 + coeff[2]*x^2 + coeff[1]*x + coeff[0] = 0
	 *
	 * if coeff[4] != 0
	 *
	 * the four roots will be stored in the complex array roots[]
	 *
	 * */
	// f64 resmin, bl311, dml3l3, aq1, bq1, cq1, dq1,aq,bq,cq,dq,d2,d3,l1,l3, errmin, gamma, del2;
	let mut acx1 = Complex::default();
	let mut bcx1 = Complex::default();
	let mut ccx1 = Complex::default();
	let mut dcx1 = Complex::default();
	let mut acx = Complex::default();
	let mut bcx = Complex::default();
	let mut ccx = Complex::default();
	let mut dcx = Complex::default();
	let mut realcase = [0; 2];
	let mut d2m = [0.; 12];
	let mut l2m = [0.; 12];
	let mut res = [0.; 12];
	let mut errv = [0.; 3];
	let mut aqv = [0.; 3];
	let mut cqv = [0.; 3];
	let mut resmin = 0.0;
	let mut err0 = 0.;
	let mut rfact = 1.0;

	if coeff[4] == 0.0 {
		println!("That's not a quartic!\n");
		return [0.; 4];
	}
	let mut a = coeff[3] / coeff[4];
	let mut b = coeff[2] / coeff[4];
	let mut c = coeff[1] / coeff[4];
	let mut d = coeff[0] / coeff[4];
	let mut phi0 = oqs_calc_phi0(a, b, c, d, false);

	// simple polynomial rescaling
	if phi0.is_nan() || phi0.is_infinite() {
		rfact = QUART_RESCAL_FACT;
		a /= rfact;
		let rfactsq = rfact * rfact;
		b /= rfactsq;
		c /= rfactsq * rfact;
		d /= rfactsq * rfactsq;
		phi0 = oqs_calc_phi0(a, b, c, d, true);
	}
	let l1 = a / 2.; /* eq. (16) */
	let l3 = b / 6. + phi0 / 2.; /* eq. (18) */
	let del2 = c - a * l3; /* defined just after eq. (27) */
	let mut nsol = 0;
	let bl311 = 2. * b / 3. - phi0 - l1 * l1; /* This is d2 as defined in eq. (20)*/
	let dml3l3 = d - l3 * l3; /* dml3l3 is d3 as defined in eq. (15) with d2=0 */
	let mut aq = 0.;
	let mut bq = 0.;
	let mut cq = 0.;
	let mut dq = 0.;
	let mut aq1 = 0.;
	let mut bq1 = 0.;
	let mut cq1 = 0.;
	let mut dq1 = 0.;

	/* Three possible solutions for d2 and l2 (see eqs. (28) and discussion which follows) */
	if bl311 != 0.0 {
		d2m[nsol] = bl311;
		l2m[nsol] = del2 / (2.0 * d2m[nsol]);
		res[nsol] = oqs_calc_err_ldlt(b, c, d, d2m[nsol], l1, l2m[nsol], l3);
		nsol += 1;
	}
	if del2 != 0. {
		l2m[nsol] = 2. * dml3l3 / del2;
		if l2m[nsol] != 0. {
			d2m[nsol] = del2 / (2. * l2m[nsol]);
			res[nsol] = oqs_calc_err_ldlt(b, c, d, d2m[nsol], l1, l2m[nsol], l3);
			nsol += 1;
		}

		d2m[nsol] = bl311;
		l2m[nsol] = 2.0 * dml3l3 / del2;
		res[nsol] = oqs_calc_err_ldlt(b, c, d, d2m[nsol], l1, l2m[nsol], l3);
		nsol += 1;
	}

	let (d2, l2) = if nsol == 0 {
		(0., 0.)
	} else {
		/* we select the (d2,l2) pair which minimizes errors */
		let mut kmin = 0;
		for k1 in 0..nsol {
			if k1 == 0 || res[k1] < resmin {
				resmin = res[k1];
				kmin = k1;
			}
		}
		(d2m[kmin], l2m[kmin])
	};

	let mut whichcase = 0;
	if d2 < 0.0 {
		/* Case I eqs. (37)-(40) */
		let gamma = (-d2).sqrt();
		aq = l1 + gamma;
		bq = l3 + gamma * l2;

		cq = l1 - gamma;
		dq = l3 - gamma * l2;
		if fabs(dq) < fabs(bq) {
			dq = d / bq;
		} else if fabs(dq) > fabs(bq) {
			bq = d / dq
		}
		if fabs(aq) < fabs(cq) {
			nsol = 0;
			if dq != 0. {
				aqv[nsol] = (c - bq * cq) / dq; /* see eqs. (47) */
				errv[nsol] = oqs_calc_err_abc(a, b, c, aqv[nsol], bq, cq, dq);
				nsol += 1;
			}
			if cq != 0. {
				aqv[nsol] = (b - dq - bq) / cq; /* see eqs. (47) */
				errv[nsol] = oqs_calc_err_abc(a, b, c, aqv[nsol], bq, cq, dq);
				nsol += 1;
			}
			aqv[nsol] = a - cq; /* see eqs. (47) */
			errv[nsol] = oqs_calc_err_abc(a, b, c, aqv[nsol], bq, cq, dq);
			nsol += 1;
			/* we select the value of aq (i.e. alpha1 in the manuscript) which minimizes errors */
			let mut kmin = 0;
			let mut errmin = 0.;
			for k in 0..nsol {
				if k == 0 || errv[k] < errmin {
					kmin = k;
					errmin = errv[k];
				}
			}
			aq = aqv[kmin];
		} else {
			nsol = 0;
			if bq != 0. {
				cqv[nsol] = (c - aq * dq) / bq; /* see eqs. (53) */
				errv[nsol] = oqs_calc_err_abc(a, b, c, aq, bq, cqv[nsol], dq);
				nsol += 1;
			}
			if aq != 0. {
				cqv[nsol] = (b - bq - dq) / aq; /* see eqs. (53) */
				errv[nsol] = oqs_calc_err_abc(a, b, c, aq, bq, cqv[nsol], dq);
				nsol += 1;
			}
			cqv[nsol] = a - aq; /* see eqs. (53) */
			errv[nsol] = oqs_calc_err_abc(a, b, c, aq, bq, cqv[nsol], dq);
			nsol += 1;
			/* we select the value of cq (i.e. alpha2 in the manuscript) which minimizes errors */
			let mut kmin = 0;
			let mut errmin = 0.;
			for k in 0..nsol {
				if k == 0 || errv[k] < errmin {
					kmin = k;
					errmin = errv[k];
				}
			}
			cq = cqv[kmin];
		}

		realcase[0] = 1;
	} else if d2 > 0. {
		/* Case II eqs. (53)-(56) */
		let gamma = sqrt(d2);
		acx = Complex::new(l1, gamma);
		bcx = Complex::new(l3, gamma * l2);
		ccx = acx.conj();
		dcx = bcx.conj();

		realcase[0] = 0;
	} else {
		realcase[0] = -1; // d2=0
	}

	/* Case III: d2 is 0 or approximately 0 (in this case check which solution is better) */
	if realcase[0] == -1 || (fabs(d2) <= MACHEPS * oqs_max3(fabs(2. * b / 3.), fabs(phi0), l1 * l1)) {
		let d3 = d - l3 * l3;
		if realcase[0] == 1 {
			err0 = oqs_calc_err_abcd(a, b, c, d, aq, bq, cq, dq);
		} else if realcase[0] == 0 {
			err0 = oqs_calc_err_abcd_cmplx(a, b, c, d, acx, bcx, ccx, dcx);
		}
		let err1 = if d3 <= 0. {
			realcase[1] = 1;
			aq1 = l1;
			bq1 = l3 + sqrt(-d3);
			cq1 = l1;
			dq1 = l3 - sqrt(-d3);
			if fabs(dq1) < fabs(bq1) {
				dq1 = d / bq1
			} else if fabs(dq1) > fabs(bq1) {
				bq1 = d / dq1
			};
			oqs_calc_err_abcd(a, b, c, d, aq1, bq1, cq1, dq1) /* eq. (68) */
		}
		// complex
		else {
			realcase[1] = 0;
			acx1 = Complex::real(l1);
			bcx1 = Complex::new(l3, sqrt(d3));
			ccx1 = Complex::real(l1);
			dcx1 = bcx1.conj();
			oqs_calc_err_abcd_cmplx(a, b, c, d, acx1, bcx1, ccx1, dcx1)
		};
		if realcase[0] == -1 || err1 < err0 {
			whichcase = 1; // d2 = 0
			if realcase[1] == 1 {
				aq = aq1;
				bq = bq1;
				cq = cq1;
				dq = dq1;
			} else {
				acx = acx1;
				bcx = bcx1;
				ccx = ccx1;
				dcx = dcx1;
			}
		}
	}
	let mut roots = if realcase[whichcase] == 1 {
		/* if alpha1, beta1, alpha2 and beta2 are real first refine
			* the coefficient through a Newton-Raphson */
		oqs_NRabcd(a, b, c, d, &mut aq, &mut bq, &mut cq, &mut dq);
		/* finally calculate the roots as roots of p1(x) and p2(x) (see end of sec. 2.1) */
		let qroots1 = oqs_solve_quadratic(aq, bq);
		let qroots2 = oqs_solve_quadratic(cq, d);
		[qroots1[0].0, qroots1[1].0, qroots2[0].0, qroots2[1].0]
	} else {
		/* complex coefficients of p1 and p2 */
		// d2!=0
		if whichcase == 0 {
			let cdiskr = acx * acx / Complex::real(4.) - bcx;
			/* calculate the roots as roots of p1(x) and p2(x) (see end of sec. 2.1) */
			let zx1 = -acx / Complex::real(2.) + csqrt(cdiskr);
			let zx2 = -acx / Complex::real(2.) - csqrt(cdiskr);
			let zxmax = if cabs(zx1) > cabs(zx2) { zx1 } else { zx2 };
			let zxmin = bcx / zxmax;
			[zxmin.0, zxmin.conj().0, zxmax.0, zxmax.conj().0]
		}
		// d2 ~ 0
		else {
			/* never gets here! */
			let cdiskr = csqrt(acx * acx - Complex::real(4.0) * bcx);
			let zx1 = Complex::real(-0.5) * (acx + cdiskr);
			let zx2 = Complex::real(-0.5) * (acx - cdiskr);
			let zxmax1 = if cabs(zx1) > cabs(zx2) { zx1 } else { zx2 };
			let zxmin1 = bcx / zxmax1;
			let cdiskr = csqrt(ccx * ccx - Complex::real(4.0) * dcx);
			let zx1 = Complex::real(-0.5) * (ccx + cdiskr);
			let zx2 = Complex::real(-0.5) * (ccx - cdiskr);
			let zxmax2 = if cabs(zx1) > cabs(zx2) { zx1 } else { zx2 };
			let zxmin2 = dcx / zxmax2;
			[zxmax1.0, zxmin1.0, zxmax2.0, zxmin2.0]
		}
	};
	if rfact != 1.0 {
		for k in 0..4 {
			roots[k] *= rfact;
		}
	}
	roots
}
