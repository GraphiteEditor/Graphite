use std::future::Future;

use crate::Node;

pub struct GetNode;

#[node_macro::node_fn(GetNode)]
async fn get_node(url: String) -> reqwest::Response {
	reqwest::get(url).await.unwrap()
}

pub struct PostNode<Body> {
	body: Body,
}

#[node_macro::node_fn(PostNode)]
async fn post_node(url: String, body: String) -> reqwest::Response {
	reqwest::Client::new().post(url).body(body).send().await.unwrap()
}

#[derive(Clone, Copy, Debug)]
pub struct EvalSyncNode {}

#[node_macro::node_fn(EvalSyncNode)]
fn eval_sync<F: Future + 'input>(future: F) -> F::Output {
	let future = futures::future::maybe_done(future);
	futures::pin_mut!(future);
	match future.as_mut().take_output() {
		Some(value) => value,
		_ => panic!("Node construction future returned pending"),
	}
}
