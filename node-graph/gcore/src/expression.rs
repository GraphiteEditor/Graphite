use crate::document::value::TaggedValue;
use graphene_core::Node;
use math_parser::evaluate;
use math_parser::value::{Number, Value};

/// A node that evaluates mathematical expressions during graph runtime
#[derive(Debug)]
pub struct ExpressionNode<T> {
    expression: T,
}

impl<'i, T: Node<'i, ()>> Node<'i, ()> for ExpressionNode<T>
where
    T::Output: AsRef<str>,
{
    type Output = TaggedValue;

    fn eval(&'i self, _input: ()) -> Self::Output {
        let expression = self.expression.eval(());
        match evaluate(expression.as_ref()) {
            Ok((Ok(value), _)) => {
                let Value::Number(num) = value;
                match num {
                    Number::Real(val) => TaggedValue::F64(val),
                    Number::Complex(c) => TaggedValue::F64(c.re),
                }
            }
            _ => TaggedValue::None,
        }
    }
}

impl<T> ExpressionNode<T> {
    pub fn new(expression: T) -> Self {
        Self { expression }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphene_core::value::ValueNode;

    #[test]
    fn test_expression_evaluation() {
        let node = ExpressionNode::new(ValueNode::new("2 + 2".to_string()));
        let result = node.eval(());
        match result {
            TaggedValue::F64(value) => assert_eq!(value, 4.0),
            _ => panic!("Expected F64 value"),
        }
    }
}
