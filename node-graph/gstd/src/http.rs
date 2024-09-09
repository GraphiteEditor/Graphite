#[node_macro::new_node_fn]
async fn get_node(_: (), url: String) -> reqwest::Response {
	reqwest::get(url).await.unwrap()
}

#[node_macro::new_node_fn]
async fn post_node(_: (), url: String, body: String) -> reqwest::Response {
	reqwest::Client::new().post(url).body(body).send().await.unwrap()
}
