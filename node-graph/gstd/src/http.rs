#[node_macro::node]
async fn get_node(_: (), url: String) -> reqwest::Response {
	reqwest::get(url).await.unwrap()
}

#[node_macro::node]
async fn post_node(_: (), url: String, body: String) -> reqwest::Response {
	reqwest::Client::new().post(url).body(body).send().await.unwrap()
}
