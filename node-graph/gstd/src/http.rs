use graphene_core::Ctx;

#[node_macro::node(category("Network"))]
async fn get_request(_: impl Ctx, url: String) -> reqwest::Response {
	reqwest::get(url).await.unwrap()
}

#[node_macro::node(category("Network"))]
async fn post_request(_: impl Ctx, url: String, body: String) -> reqwest::Response {
	reqwest::Client::new().post(url).body(body).send().await.unwrap()
}
