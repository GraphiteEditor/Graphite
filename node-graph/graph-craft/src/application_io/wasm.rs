use dyn_any::StaticType;
use graphene_application_io::{ApplicationError, ApplicationIo, ResourceFuture};
use std::collections::HashMap;
use std::sync::Arc;
#[cfg(feature = "tokio")]
use tokio::io::AsyncReadExt;
#[cfg(target_family = "wasm")]
use wasm_bindgen::JsCast;
#[cfg(feature = "wgpu")]
use wgpu_executor::WgpuExecutor;

#[derive(Debug, Default)]
pub struct WasmApplicationIo {
	#[cfg(feature = "wgpu")]
	pub(crate) gpu_executor: Option<WgpuExecutor>,
	pub resources: HashMap<String, Arc<[u8]>>,
}

impl WasmApplicationIo {
	pub async fn new() -> Self {
		#[cfg(feature = "wgpu")]
		let executor = if let Some(gpu) = web_sys::window().map(|w| w.navigator().gpu()) {
			let request_adapter = || {
				let request_adapter = js_sys::Reflect::get(&gpu, &wasm_bindgen::JsValue::from_str("requestAdapter")).ok()?;
				let function = request_adapter.dyn_ref::<js_sys::Function>()?;
				function.call0(&gpu).ok()
			};
			let result = request_adapter();
			match result {
				None => None,
				Some(_) => WgpuExecutor::new().await,
			}
		} else {
			None
		};

		#[cfg(not(feature = "wgpu"))]
		let wgpu_available = false;
		#[cfg(feature = "wgpu")]
		let wgpu_available = executor.is_some();
		super::set_wgpu_available(wgpu_available);

		let mut io = Self {
			#[cfg(feature = "wgpu")]
			gpu_executor: executor,
			resources: HashMap::new(),
		};
		io.resources.insert("null".to_string(), Arc::from(include_bytes!("../null.png").to_vec()));

		io
	}
}

impl ApplicationIo for WasmApplicationIo {
	#[cfg(feature = "wgpu")]
	type Executor = WgpuExecutor;
	#[cfg(not(feature = "wgpu"))]
	type Executor = ();

	#[cfg(feature = "wgpu")]
	fn gpu_executor(&self) -> Option<&Self::Executor> {
		self.gpu_executor.as_ref()
	}

	fn load_resource(&self, url: impl AsRef<str>) -> Result<ResourceFuture, ApplicationError> {
		let url = url::Url::parse(url.as_ref()).map_err(|_| ApplicationError::InvalidUrl)?;
		log::trace!("Loading resource: {url:?}");
		match url.scheme() {
			#[cfg(feature = "tokio")]
			"file" => {
				let path = url.to_file_path().map_err(|_| ApplicationError::NotFound)?;
				let path = path.to_str().ok_or(ApplicationError::NotFound)?;
				let path = path.to_owned();
				Ok(Box::pin(async move {
					let file = tokio::fs::File::open(path).await.map_err(|_| ApplicationError::NotFound)?;
					let mut reader = tokio::io::BufReader::new(file);
					let mut data = Vec::new();
					reader.read_to_end(&mut data).await.map_err(|_| ApplicationError::NotFound)?;
					Ok(Arc::from(data))
				}) as ResourceFuture)
			}
			"http" | "https" => {
				let url = url.to_string();
				Ok(Box::pin(async move {
					let client = reqwest::Client::new();
					let response = client.get(url).send().await.map_err(|_| ApplicationError::NotFound)?;
					let data = response.bytes().await.map_err(|_| ApplicationError::NotFound)?;
					Ok(Arc::from(data.to_vec()))
				}) as ResourceFuture)
			}
			"graphite" => {
				let path = url.path();
				let path = path.to_owned();
				log::trace!("Loading local resource: {path}");
				let data = self.resources.get(&path).ok_or(ApplicationError::NotFound)?.clone();
				Ok(Box::pin(async move { Ok(data.clone()) }) as ResourceFuture)
			}
			_ => Err(ApplicationError::NotFound),
		}
	}
}

unsafe impl StaticType for WasmApplicationIo {
	type Static = WasmApplicationIo;
}
