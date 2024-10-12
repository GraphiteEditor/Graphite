use crate::value::Value;

#[derive(Debug, PartialEq)]
pub enum Literal {
    Int(u64),
    Float(f64),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

#[derive(Debug, PartialEq)]
pub enum UnaryOp {
    Neg,
    Sqrt,
    Sin,
    Cos,
    Tan,
    Csc,
    Sec,
    Cot,
    InvSin,
    InvCos,
    InvTan,
    InvCsc,
    InvSec,
    InvCot,
    Fac,
}

#[derive(Debug, PartialEq)]
pub enum Node {
    Lit(Literal),
    Var(String),
    FnCall { name: String, expr: Box<Node> },
    GlobalVar(String),
    BinOp { lhs: Box<Node>, op: BinaryOp, rhs: Box<Node> },
    UnaryOp { expr: Box<Node>, op: UnaryOp },
}

impl Node {
    pub fn eval(&self) -> Value {
        match self {
            Node::Lit(lit) => match lit {
                Literal::Int(i) => Value::from_f64(*i as f64),
                Literal::Float(f) => Value::from_f64(*f),
            },

            Node::BinOp { lhs, op, rhs } => {
                let left = lhs.eval();
                let right = rhs.eval();

                // Ensure we can handle complex numbers
                let (left_real, left_imag) = match left {
                    Value::Complex(real, imag) => (real, imag),
                };
                let (right_real, right_imag) = match right {
                    Value::Complex(real, imag) => (real, imag),
                };

                let result = match op {
                    BinaryOp::Add => Value::Complex(left_real + right_real, left_imag + right_imag),
                    BinaryOp::Sub => Value::Complex(left_real - right_real, left_imag - right_imag),
                    BinaryOp::Mul => Value::Complex(left_real * right_real - left_imag * right_imag, left_real * right_imag + left_imag * right_real),
                    BinaryOp::Div => {
                        let denom = right_real.powi(2) + right_imag.powi(2);
                        Value::Complex((left_real * right_real + left_imag * right_imag) / denom, (left_imag * right_real - left_real * right_imag) / denom)
                    }
                    BinaryOp::Pow => {

                        panic!("Power operation for complex numbers is not implemented");
                    }
                };
                result
            }

            Node::UnaryOp { expr, op } => {
                let value = expr.eval();
                let (real, imag) = match value {
                    Value::Complex(real, imag) => (real, imag),
                };

                let result = match op {
                    UnaryOp::Neg => Value::Complex(-real, -imag),
                    UnaryOp::Sqrt => {
                        let r = (real.powi(2) + imag.powi(2)).sqrt();
                        let theta = (imag / real).atan();
                        let sqrt_r = r.sqrt();
                        Value::Complex(sqrt_r * theta.cos(), sqrt_r * theta.sin())
                    }
                    UnaryOp::Sin => Value::Complex(real.sin() * imag.cosh(), real.cos() * imag.sinh()),
                    UnaryOp::Cos => Value::Complex(real.cos() * imag.cosh(), -real.sin() * imag.sinh()),
                    UnaryOp::Tan => {
                        let denom = (2.0 * real.cos()).cosh();
                        Value::Complex((2.0 * real.sin() * imag.cosh()) / denom, (2.0 * imag.sin() * real.cos()) / denom)
                    }
                    _ => panic!("Unary operation not implemented for complex numbers"),
                };
                result
            }
			Node::Var(_) => unimplemented!("Variable accses not implemented"),
			Node::GlobalVar(_) => unimplemented!("Global variable accses not implemented")
			Node::FnCall{..} => unimplemented!("Function calls not implemented")
        }
    }
}

fn factorial(value: f64) -> f64 {
    if value < 0.0 || value.fract() != 0.0 {
        panic!("Factorial is not defined for negative or non-integer values");
    }
    (1..=(value as u64)).fold(1u64, |acc, x| acc * x) as f64
}

#[cfg(test)]
mod tests {
    use crate::{
        ast::{BinaryOp, Literal, Node, UnaryOp},
        value::Value,
    };

    macro_rules! eval_tests {
		($($name:ident: $expected:expr => $expr:expr),* $(,)?) => {
			$(
				#[test]
				fn $name() {
					let result = $expr.eval();
					assert_eq!(result, $expected);
				}
			)*
		};
	}

    eval_tests! {
        test_addition: Value::from_f64(7.0) => Node::BinOp {
            lhs: Box::new(Node::Lit(Literal::Int(3))),
            op: BinaryOp::Add,
            rhs: Box::new(Node::Lit(Literal::Int(4))),
        },
        test_subtraction: Value::from_f64(1.0) => Node::BinOp {
            lhs: Box::new(Node::Lit(Literal::Int(5))),
            op: BinaryOp::Sub,
            rhs: Box::new(Node::Lit(Literal::Int(4))),
        },
        test_multiplication: Value::from_f64(12.0) => Node::BinOp {
            lhs: Box::new(Node::Lit(Literal::Int(3))),
            op: BinaryOp::Mul,
            rhs: Box::new(Node::Lit(Literal::Int(4))),
        },
        test_division: Value::from_f64(2.5) => Node::BinOp {
            lhs: Box::new(Node::Lit(Literal::Float(5.0))),
            op: BinaryOp::Div,
            rhs: Box::new(Node::Lit(Literal::Int(2))),
        },
        test_negation: Value::from_f64(-3.0) => Node::UnaryOp {
            val: Box::new(Node::Lit(Literal::Int(3))),
            op: UnaryOp::Neg,
        },
        test_sqrt: Value::from_f64(2.0) => Node::UnaryOp {
            val: Box::new(Node::Lit(Literal::Int(4))),
            op: UnaryOp::Sqrt,
        },
        test_sine: Value::from_f64(0.0) => Node::UnaryOp {
            val: Box::new(Node::Lit(Literal::Float(0.0))),
            op: UnaryOp::Sin,
        },
        test_cosine: Value::from_f64(1.0) => Node::UnaryOp {
            val: Box::new(Node::Lit(Literal::Float(0.0))),
            op: UnaryOp::Cos,
        },
        test_power: Value::from_f64(8.0) => Node::BinOp {
            lhs: Box::new(Node::Lit(Literal::Int(2))),
            op: BinaryOp::Pow,
            rhs: Box::new(Node::Lit(Literal::Int(3))),
        },
    }
}
