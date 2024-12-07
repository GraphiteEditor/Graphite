use math_parser::evaluate;
use math_parser::value::{Number, Value};

#[node_macro::node(category("Math"))]
/// A node that evaluates mathematical expressions during graph runtime
fn expression_node(
    _input: (),  // Empty input for now
    #[property(name = "Expression", exposed = true, value_source = "UserInput")]
	expression: String  // UI editable props
) -> f64 {
    match evaluate(&expression) {
        Ok((Ok(value), _)) => {
            let Value::Number(num) = value;
            match num {
                Number::Real(val) => val,
                Number::Complex(c) => c.re,
            }
        }
        _ => 0.0  // Better error handling
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expression_evaluation() {
        let result = expression_node((), "2 + 2".to_string());
        assert_eq!(result, 4.0);
    }
}
