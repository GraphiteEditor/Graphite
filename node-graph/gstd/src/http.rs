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
