use core_types::context::{CloneVarArgs, ExtractAll};
use core_types::list::{Bundle, Item};
use core_types::{Color, Context, Ctx, OwnedContextImpl, Poll};
use glam::{DAffine2, DVec2};
use graphic_types::vector_types::Gradient;
use graphic_types::{Artboard, Graphic, Vector};
use raster_types::{CPU, GPU, Raster};

/// Shows the placeholder until the content has finished loading.
#[node_macro::node(category("Routing"))]
fn placeholder<T>(
	ctx: impl Ctx + CloneVarArgs + ExtractAll,
	#[expose]
	#[implementations(
		Context -> Item<String>,
		Context -> Item<bool>,
		Context -> Item<f64>,
		Context -> Item<i64>,
		Context -> Item<DVec2>,
		Context -> Item<DAffine2>,
		Context -> Item<Vector>,
		Context -> Item<Graphic>,
		Context -> Item<Raster<CPU>>,
		Context -> Item<Raster<GPU>>,
		Context -> Item<Color>,
		Context -> Item<Gradient>,
		Context -> Item<Artboard>,
		Context -> Item<Bundle<String>>,
		Context -> Item<Bundle<bool>>,
		Context -> Item<Bundle<f64>>,
		Context -> Item<Bundle<i64>>,
		Context -> Item<Bundle<DVec2>>,
		Context -> Item<Bundle<DAffine2>>,
		Context -> Item<Bundle<Vector>>,
		Context -> Item<Bundle<Graphic>>,
		Context -> Item<Bundle<Raster<CPU>>>,
		Context -> Item<Bundle<Raster<GPU>>>,
		Context -> Item<Bundle<Color>>,
		Context -> Item<Bundle<Gradient>>,
		Context -> Item<Bundle<Artboard>>,
	)]
	content: impl Node<Context<'static>, Output = T>,
	#[implementations(
		Context -> Item<String>,
		Context -> Item<bool>,
		Context -> Item<f64>,
		Context -> Item<i64>,
		Context -> Item<DVec2>,
		Context -> Item<DAffine2>,
		Context -> Item<Vector>,
		Context -> Item<Graphic>,
		Context -> Item<Raster<CPU>>,
		Context -> Item<Raster<GPU>>,
		Context -> Item<Color>,
		Context -> Item<Gradient>,
		Context -> Item<Artboard>,
		Context -> Item<Bundle<String>>,
		Context -> Item<Bundle<bool>>,
		Context -> Item<Bundle<f64>>,
		Context -> Item<Bundle<i64>>,
		Context -> Item<Bundle<DVec2>>,
		Context -> Item<Bundle<DAffine2>>,
		Context -> Item<Bundle<Vector>>,
		Context -> Item<Bundle<Graphic>>,
		Context -> Item<Bundle<Raster<CPU>>>,
		Context -> Item<Bundle<Raster<GPU>>>,
		Context -> Item<Bundle<Color>>,
		Context -> Item<Bundle<Gradient>>,
		Context -> Item<Bundle<Artboard>>,
	)]
	placeholder: impl Node<Context<'static>, Output = T>,
) -> Poll<T> {
	let ctx = OwnedContextImpl::from(ctx).into_context();

	match content.eval(&ctx) {
		Poll::Pending | Poll::Partial(_) => match placeholder.eval(&ctx) {
			Poll::Final(value) | Poll::Partial(value) => Poll::Partial(value),
			Poll::Fallback(boxed) => Poll::Partial(boxed.0),
			state => state,
		},
		state => state,
	}
}

/// Keeps rendering with the fallback where the content failed, and still shows the error.
#[node_macro::node(category("Routing"))]
fn fallback<T>(
	ctx: impl Ctx + CloneVarArgs + ExtractAll,
	#[expose]
	#[implementations(
		Context -> Item<String>,
		Context -> Item<bool>,
		Context -> Item<f64>,
		Context -> Item<i64>,
		Context -> Item<DVec2>,
		Context -> Item<DAffine2>,
		Context -> Item<Vector>,
		Context -> Item<Graphic>,
		Context -> Item<Raster<CPU>>,
		Context -> Item<Raster<GPU>>,
		Context -> Item<Color>,
		Context -> Item<Gradient>,
		Context -> Item<Artboard>,
		Context -> Item<Bundle<String>>,
		Context -> Item<Bundle<bool>>,
		Context -> Item<Bundle<f64>>,
		Context -> Item<Bundle<i64>>,
		Context -> Item<Bundle<DVec2>>,
		Context -> Item<Bundle<DAffine2>>,
		Context -> Item<Bundle<Vector>>,
		Context -> Item<Bundle<Graphic>>,
		Context -> Item<Bundle<Raster<CPU>>>,
		Context -> Item<Bundle<Raster<GPU>>>,
		Context -> Item<Bundle<Color>>,
		Context -> Item<Bundle<Gradient>>,
		Context -> Item<Bundle<Artboard>>,
	)]
	content: impl Node<Context<'static>, Output = T>,
	#[implementations(
		Context -> Item<String>,
		Context -> Item<bool>,
		Context -> Item<f64>,
		Context -> Item<i64>,
		Context -> Item<DVec2>,
		Context -> Item<DAffine2>,
		Context -> Item<Vector>,
		Context -> Item<Graphic>,
		Context -> Item<Raster<CPU>>,
		Context -> Item<Raster<GPU>>,
		Context -> Item<Color>,
		Context -> Item<Gradient>,
		Context -> Item<Artboard>,
		Context -> Item<Bundle<String>>,
		Context -> Item<Bundle<bool>>,
		Context -> Item<Bundle<f64>>,
		Context -> Item<Bundle<i64>>,
		Context -> Item<Bundle<DVec2>>,
		Context -> Item<Bundle<DAffine2>>,
		Context -> Item<Bundle<Vector>>,
		Context -> Item<Bundle<Graphic>>,
		Context -> Item<Bundle<Raster<CPU>>>,
		Context -> Item<Bundle<Raster<GPU>>>,
		Context -> Item<Bundle<Color>>,
		Context -> Item<Bundle<Gradient>>,
		Context -> Item<Bundle<Artboard>>,
	)]
	fallback: impl Node<Context<'static>, Output = T>,
) -> Poll<T> {
	let ctx = OwnedContextImpl::from(ctx).into_context();

	let error = match content.eval(&ctx) {
		Poll::Fallback(boxed) => boxed.1,
		Poll::Error(error) => *error,
		state => return state,
	};
	match fallback.eval(&ctx) {
		Poll::Final(value) => Poll::Fallback(Box::new((value, error))),
		Poll::Fallback(boxed) => Poll::Fallback(Box::new((boxed.0, error))),
		state => state,
	}
}

/// Uses the alternative where the content failed, and hides the error.
#[node_macro::node(category("Routing"))]
fn catch<T>(
	ctx: impl Ctx + CloneVarArgs + ExtractAll,
	#[expose]
	#[implementations(
		Context -> Item<String>,
		Context -> Item<bool>,
		Context -> Item<f64>,
		Context -> Item<i64>,
		Context -> Item<DVec2>,
		Context -> Item<DAffine2>,
		Context -> Item<Vector>,
		Context -> Item<Graphic>,
		Context -> Item<Raster<CPU>>,
		Context -> Item<Raster<GPU>>,
		Context -> Item<Color>,
		Context -> Item<Gradient>,
		Context -> Item<Artboard>,
		Context -> Item<Bundle<String>>,
		Context -> Item<Bundle<bool>>,
		Context -> Item<Bundle<f64>>,
		Context -> Item<Bundle<i64>>,
		Context -> Item<Bundle<DVec2>>,
		Context -> Item<Bundle<DAffine2>>,
		Context -> Item<Bundle<Vector>>,
		Context -> Item<Bundle<Graphic>>,
		Context -> Item<Bundle<Raster<CPU>>>,
		Context -> Item<Bundle<Raster<GPU>>>,
		Context -> Item<Bundle<Color>>,
		Context -> Item<Bundle<Gradient>>,
		Context -> Item<Bundle<Artboard>>,
	)]
	content: impl Node<Context<'static>, Output = T>,
	#[implementations(
		Context -> Item<String>,
		Context -> Item<bool>,
		Context -> Item<f64>,
		Context -> Item<i64>,
		Context -> Item<DVec2>,
		Context -> Item<DAffine2>,
		Context -> Item<Vector>,
		Context -> Item<Graphic>,
		Context -> Item<Raster<CPU>>,
		Context -> Item<Raster<GPU>>,
		Context -> Item<Color>,
		Context -> Item<Gradient>,
		Context -> Item<Artboard>,
		Context -> Item<Bundle<String>>,
		Context -> Item<Bundle<bool>>,
		Context -> Item<Bundle<f64>>,
		Context -> Item<Bundle<i64>>,
		Context -> Item<Bundle<DVec2>>,
		Context -> Item<Bundle<DAffine2>>,
		Context -> Item<Bundle<Vector>>,
		Context -> Item<Bundle<Graphic>>,
		Context -> Item<Bundle<Raster<CPU>>>,
		Context -> Item<Bundle<Raster<GPU>>>,
		Context -> Item<Bundle<Color>>,
		Context -> Item<Bundle<Gradient>>,
		Context -> Item<Bundle<Artboard>>,
	)]
	alternative: impl Node<Context<'static>, Output = T>,
) -> Poll<T> {
	let ctx = OwnedContextImpl::from(ctx).into_context();

	match content.eval(&ctx) {
		Poll::Fallback(_) | Poll::Error(_) => alternative.eval(&ctx),
		state => state,
	}
}

/// Keeps the value a failed node still gave, and hides its error. A failure without a value still stops everything that depends on it.
#[node_macro::node(category("Routing"))]
fn suppress_error<T>(
	ctx: impl Ctx + CloneVarArgs + ExtractAll,
	#[expose]
	#[implementations(
		Context -> Item<String>,
		Context -> Item<bool>,
		Context -> Item<f64>,
		Context -> Item<i64>,
		Context -> Item<DVec2>,
		Context -> Item<DAffine2>,
		Context -> Item<Vector>,
		Context -> Item<Graphic>,
		Context -> Item<Raster<CPU>>,
		Context -> Item<Raster<GPU>>,
		Context -> Item<Color>,
		Context -> Item<Gradient>,
		Context -> Item<Artboard>,
		Context -> Item<Bundle<String>>,
		Context -> Item<Bundle<bool>>,
		Context -> Item<Bundle<f64>>,
		Context -> Item<Bundle<i64>>,
		Context -> Item<Bundle<DVec2>>,
		Context -> Item<Bundle<DAffine2>>,
		Context -> Item<Bundle<Vector>>,
		Context -> Item<Bundle<Graphic>>,
		Context -> Item<Bundle<Raster<CPU>>>,
		Context -> Item<Bundle<Raster<GPU>>>,
		Context -> Item<Bundle<Color>>,
		Context -> Item<Bundle<Gradient>>,
		Context -> Item<Bundle<Artboard>>,
	)]
	content: impl Node<Context<'static>, Output = T>,
) -> Poll<T> {
	match content.eval(&OwnedContextImpl::from(ctx).into_context()) {
		Poll::Fallback(boxed) => Poll::Final(boxed.0),
		state => state,
	}
}

/// Raises an error with the message while the condition is true, so a custom node can reject input it can't work with.
#[node_macro::node(category("Routing"))]
fn throw_error<T>(
	_: impl Ctx,
	#[implementations(
		Item<String>,
		Item<bool>,
		Item<f64>,
		Item<i64>,
		Item<DVec2>,
		Item<DAffine2>,
		Item<Vector>,
		Item<Graphic>,
		Item<Raster<CPU>>,
		Item<Raster<GPU>>,
		Item<Color>,
		Item<Gradient>,
		Item<Artboard>,
		Item<Bundle<String>>,
		Item<Bundle<bool>>,
		Item<Bundle<f64>>,
		Item<Bundle<i64>>,
		Item<Bundle<DVec2>>,
		Item<Bundle<DAffine2>>,
		Item<Bundle<Vector>>,
		Item<Bundle<Graphic>>,
		Item<Bundle<Raster<CPU>>>,
		Item<Bundle<Raster<GPU>>>,
		Item<Bundle<Color>>,
		Item<Bundle<Gradient>>,
		Item<Bundle<Artboard>>,
	)]
	value: T,
	condition: Item<bool>,
	#[default("Error")] message: Item<String>,
	keep_value: Item<bool>,
) -> Poll<T> {
	if !*condition.element() {
		return Poll::Final(value);
	}

	let message = message.into_element();
	if *keep_value.element() { Poll::fallback(value, message) } else { Poll::error(message) }
}

#[cfg(test)]
mod test {
	use super::*;
	use core_types::Node;

	struct Fixed(Poll<Item<f64>>);

	impl Node<Context<'static>> for Fixed {
		type Output = Item<f64>;
		fn eval(&self, _: &Context<'static>) -> Poll<Item<f64>> {
			self.0.clone()
		}
	}

	fn item(value: f64) -> Item<f64> {
		Item::new_from_element(value)
	}

	fn eval<N: Node<Context<'static>>>(node: N) -> Poll<N::Output> {
		node.eval(&Context::default())
	}

	fn error_of<T>(state: Poll<T>) -> (String, Vec<usize>) {
		let error = match state {
			Poll::Fallback(boxed) => boxed.1,
			Poll::Error(error) => *error,
			_ => panic!("Expected an error"),
		};
		(error.to_string(), error.trace().to_vec())
	}

	#[test]
	fn placeholder_replaces_only_unfinished_content() {
		let placeholder = |content| eval(PlaceholderNode::new(Fixed(content), Fixed(Poll::Final(item(2.)))));
		assert_eq!(placeholder(Poll::Pending), Poll::Partial(item(2.)));
		assert_eq!(placeholder(Poll::Partial(item(1.))), Poll::Partial(item(2.)));
		assert_eq!(placeholder(Poll::Final(item(1.))), Poll::Final(item(1.)));
		assert_eq!(error_of(placeholder(Poll::error("failed"))), ("failed".to_string(), vec![0]));
	}

	#[test]
	fn fallback_keeps_the_content_error() {
		let fallback = |content| eval(FallbackNode::new(Fixed(content), Fixed(Poll::Final(item(2.)))));
		assert_eq!(fallback(Poll::Final(item(1.))), Poll::Final(item(1.)));
		assert_eq!(fallback(Poll::Pending), Poll::Pending);

		let Poll::Fallback(boxed) = fallback(Poll::error("failed")) else {
			panic!("A failure should fall back")
		};
		assert_eq!(boxed.0, item(2.));
		assert_eq!(error_of(Poll::Fallback(boxed)), ("failed".to_string(), vec![0]));

		let Poll::Fallback(boxed) = fallback(Poll::fallback(item(1.), "own stand-in")) else {
			panic!("A failure should fall back")
		};
		assert_eq!(boxed.0, item(2.), "The fallback replaces the failed node's own stand-in");
		assert_eq!(error_of(Poll::Fallback(boxed)), ("own stand-in".to_string(), vec![0]));

		let provisional = eval(FallbackNode::new(Fixed(Poll::error("failed")), Fixed(Poll::Partial(item(2.)))));
		assert_eq!(provisional, Poll::Partial(item(2.)), "An error with a provisional value is held back");
	}

	#[test]
	fn catch_uses_the_alternative_where_the_content_failed() {
		let catch = |content| eval(CatchNode::new(Fixed(content), Fixed(Poll::Final(item(2.)))));
		assert_eq!(catch(Poll::Final(item(1.))), Poll::Final(item(1.)));
		assert_eq!(catch(Poll::Partial(item(1.))), Poll::Partial(item(1.)));
		assert_eq!(catch(Poll::Pending), Poll::Pending);
		assert_eq!(catch(Poll::fallback(item(1.), "stand-in")), Poll::Final(item(2.)));
		assert_eq!(catch(Poll::error("failed")), Poll::Final(item(2.)));

		let failing_alternative = eval(CatchNode::new(Fixed(Poll::error("first")), Fixed(Poll::error("second"))));
		assert_eq!(error_of(failing_alternative), ("second".to_string(), vec![1]));
	}

	#[test]
	fn suppress_error_hides_only_an_error_with_a_value() {
		let suppress = |content| eval(SuppressErrorNode::new(Fixed(content)));
		assert_eq!(suppress(Poll::fallback(item(1.), "stand-in")), Poll::Final(item(1.)));
		assert_eq!(suppress(Poll::Partial(item(1.))), Poll::Partial(item(1.)));
		assert_eq!(error_of(suppress(Poll::error("failed"))), ("failed".to_string(), vec![0]));
	}

	#[test]
	fn throw_error_stops_or_keeps_the_value() {
		let message = || Item::new_from_element("too small".to_string());
		let flag = Item::new_from_element;

		assert_eq!(throw_error((), item(1.), flag(false), message(), flag(false)), Poll::Final(item(1.)));
		assert_eq!(error_of(throw_error((), item(1.), flag(true), message(), flag(false))), ("too small".to_string(), vec![]));
		let Poll::Fallback(boxed) = throw_error((), item(1.), flag(true), message(), flag(true)) else {
			panic!("Keeping the value should fall back")
		};
		assert_eq!(boxed.0, item(1.));
		assert_eq!(boxed.1.to_string(), "too small");
	}
}
