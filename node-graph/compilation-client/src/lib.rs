use gpu_compiler_bin_wrapper::CompileRequest;
use graph_craft::document::*;

pub async fn compile<I, O>(network: NodeNetwork) -> Result<Vec<u8>, reqwest::Error> {
	let client = reqwest::Client::new();

	let compile_request = CompileRequest::new(network, std::any::type_name::<I>().to_owned(), std::any::type_name::<O>().to_owned());
	let response = client.post("http://localhost:3000/compile/spriv").json(&compile_request).send();
	let response = response.await?;
	response.bytes().await.map(|b| b.to_vec())
}

pub fn compile_sync<I: 'static, O: 'static>(network: NodeNetwork) -> Result<Vec<u8>, reqwest::Error> {
	future_executor::block_on(compile::<I, O>(network))
}
