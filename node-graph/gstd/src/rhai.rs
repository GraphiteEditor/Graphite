use graph_craft::{
	document::value::TaggedValue,
	proto::{Any, FutureAny},
};
use graphene_core::{Context, Node};
use rhai::{Engine, Scope};

// For Serde conversion
use rhai::serde::{from_dynamic, to_dynamic};

pub struct RhaiNode<Source, Input> {
	source: Source,
	input: Input,
}

impl<'n, S, I> Node<'n, Any<'n>> for RhaiNode<S, I>
where
	S: Node<'n, Any<'n>, Output = FutureAny<'n>>,
	I: Node<'n, Any<'n>, Output = FutureAny<'n>>,
{
	type Output = FutureAny<'n>;

	fn eval(&'n self, ctx: Any<'n>) -> Self::Output {
		let ctx: Box<Context> = dyn_any::downcast(ctx).unwrap();
		let source = self.source.eval(ctx.clone());
		let input = self.input.eval(ctx);
		Box::pin(async move {
			// Get the script source and input value
			let source = source.await;
			let input = input.await;

			// Convert to appropriate types
			let script: String = match dyn_any::downcast::<String>(source) {
				Ok(script) => *script,
				Err(err) => {
					log::error!("Failed to convert script source to String: {}", err);
					return Box::new(()) as Any<'n>;
				}
			};

			let tagged_value = match TaggedValue::try_from_any(input) {
				Ok(value) => value,
				Err(err) => {
					log::error!("Failed to convert input to TaggedValue: {}", err);
					return Box::new(()) as Any<'n>;
				}
			};

			// Set up Rhai engine
			let mut engine = Engine::new();

			// Register any additional utility functions
			register_utility_functions(&mut engine);

			// Create a scope and add the input value
			let mut scope = Scope::new();

			// Convert TaggedValue to appropriate Rhai type
			// This is the key part we need to fix
			match tagged_value {
				TaggedValue::F64(val) => {
					// Directly push as primitive f64
					scope.push("input", val);
				}
				TaggedValue::U64(val) => {
					// Convert to i64 which Rhai uses for integers
					scope.push("input", val as i64);
				}
				TaggedValue::U32(val) => {
					// Convert to i64 which Rhai uses for integers
					scope.push("input", val as i64);
				}
				TaggedValue::Bool(val) => {
					scope.push("input", val);
				}
				TaggedValue::String(val) => {
					scope.push("input", val.clone());
				}
				// For complex types, use Serde conversion
				_ => match to_dynamic(tagged_value.clone()) {
					Ok(dynamic) => {
						scope.push("input", dynamic);
					}
					Err(err) => {
						log::error!("Failed to convert input to Rhai Dynamic: {}", err);
						return Box::new(()) as Any<'n>;
					}
				},
			}

			// Evaluate the script
			match engine.eval_with_scope::<rhai::Dynamic>(&mut scope, &script) {
				Ok(result) => {
					// Convert Rhai result back to TaggedValue
					if result.is::<f64>() {
						let val = result.cast::<f64>();
						TaggedValue::F64(val).to_any()
					} else if result.is::<i64>() {
						let val = result.cast::<i64>();
						TaggedValue::F64(val as f64).to_any()
					} else if result.is::<bool>() {
						let val = result.cast::<bool>();
						TaggedValue::Bool(val).to_any()
					} else if result.is::<String>() {
						let val = result.cast::<String>();
						TaggedValue::String(val).to_any()
					} else {
						// For complex types, use Serde conversion
						match from_dynamic(&result) {
							Ok(value) => TaggedValue::to_any(value),
							Err(err) => {
								log::error!("Failed to convert Rhai result to TaggedValue: {}", err);
								Box::new(()) as Any<'n>
							}
						}
					}
				}
				Err(err) => {
					log::error!("Rhai script evaluation error: {}", err);
					Box::new(()) as Any<'n>
				}
			}
		})
	}
}

// Register utility functions that would be useful in scripts
fn register_utility_functions(engine: &mut Engine) {
	// Logging function
	engine.register_fn("log", |msg: &str| {
		log::info!("Rhai script log: {}", msg);
	});
}

impl<S, I> RhaiNode<S, I> {
	pub fn new(input: I, source: S) -> RhaiNode<S, I> {
		RhaiNode { source, input }
	}
}
