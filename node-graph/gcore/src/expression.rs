use math_parser::evaluate;
use math_parser::value::{Number, Value};

/// A node that evaluates mathematical expressions during graph runtime.
#[node_macro::node(category("Math"))]
fn expression_node(
    _: (),
    #[implementations(f64, f64)]
    input: f64,
    #[implementations(String, String)]
    #[default("0")]
    expression: String,
) -> f64 {
    match evaluate(&expression) {
        Ok((Ok(value), _)) => {
            let Value::Number(num) = value;
            match num {
                Number::Real(val) => val,
                Number::Complex(c) => c.re,
            }
        }
        _ => input
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_expression() {
        let result = expression_node((), 0.0, "2 + 2".to_string());
        assert_eq!(result, 4.0);
    }

    #[test]
    fn test_complex_expression() {
        let result = expression_node((), 0.0, "(5 * 3) + (10 / 2)".to_string());
        assert_eq!(result, 20.0);
    }

    #[test]
    fn test_default_expression() {
        let result = expression_node((), 5.0, "0".to_string());
        assert_eq!(result, 0.0);
    }

    #[test]
    fn test_invalid_expression() {
        let input = 5.0;
        let result = expression_node((), input, "invalid".to_string());
        assert_eq!(result, input);
    }
}
