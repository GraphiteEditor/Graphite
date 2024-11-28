use crate::document::value::TaggedValue;
use dyn_any::DynAny;
use graphene_core::Node;
use math_parser::evaluate;
use math_parser::value::{Number, Value};
use std::future::Future;
use std::pin::Pin;

/// A node that evaluates mathematical expressions during graph runtime
#[derive(Debug)]
pub struct ExpressionNode {
    /// The mathematical expression to evaluate
    expression: String,
}

impl ExpressionNode {
    pub fn new(expression: String) -> Self {
        Self { expression }
    }
}

impl<'input> Node<'input, Box<dyn DynAny<'input> + Send>> for ExpressionNode {
    type Output = Pin<Box<dyn Future<Output = Box<dyn DynAny<'input> + Send>> + Send + 'input>>;

    fn eval(&'input self, _: Box<dyn DynAny<'input> + Send>) -> Self::Output {
        let expression = self.expression.clone();
        Box::pin(async move {
            match evaluate(&expression) {
                Ok((Ok(value), _)) => {
                    let Value::Number(num) = value;
                    match num {
                        Number::Real(val) => Box::new(TaggedValue::F64(val)) as Box<dyn DynAny<'input> + Send>,
                        Number::Complex(c) => Box::new(TaggedValue::F64(c.re)) as Box<dyn DynAny<'input> + Send>,
                    }
                }
                _ => Box::new(TaggedValue::None) as Box<dyn DynAny<'input> + Send>,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dyn_any::downcast_ref;

    #[test]
    fn test_expression_evaluation() {
        let node = ExpressionNode::new("2 + 2".to_string());
        let future = node.eval(Box::<()>::default());
        let result = futures::executor::block_on(future);
        let tagged_value = downcast_ref::<TaggedValue>(result.as_ref()).unwrap();
        match tagged_value {
            TaggedValue::F64(value) => {
                assert_eq!(value, 4.0);
            }
            _ => panic!("Expected F64 value"),
        }
    }
}
