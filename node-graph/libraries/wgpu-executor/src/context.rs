use wgpu::{Backends, Device, Features};
#[cfg(target_family = "wasm")]
use wgpu_sync::Surface;
use wgpu_sync::{Adapter, Instance, Queue};

/// Features the renderer makes use of where the GPU has them, besides any a builder asks for, to render content that effects go on to process into 16-bit unorm textures.
const RENDERER_OPTIONAL_FEATURES: Features = Features::TEXTURE_FORMAT_16BIT_NORM.union(Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES);

#[derive(Debug, Clone)]
pub struct Context {
	pub device: Device,
	pub queue: Queue,
	pub instance: Instance,
	pub adapter: Adapter,
}

#[cfg(not(target_family = "wasm"))]
impl Context {
	pub async fn new() -> Option<Self> {
		ContextBuilder::new().build().await
	}

	/// The context the GPU tests run on, which wgpu's environment variables can aim at each backend and shader compiler in turn.
	#[cfg(test)]
	pub(crate) async fn for_tests() -> Option<Self> {
		let context = ContextBuilder::new().with_environment_overrides().build().await;

		// A backend asked for by name fails the tests if it can't run them, rather than passing them all as skipped
		assert!(
			context.is_some() || std::env::var_os("WGPU_BACKEND").is_none(),
			"No device could be made on the backend WGPU_BACKEND names"
		);

		context
	}
}

#[derive(Default)]
pub struct ContextBuilder {
	backends: Backends,
	features: Features,
	optional_features: Features,
	selection: Option<usize>,
	environment_overrides: bool,
}
impl ContextBuilder {
	pub fn new() -> Self {
		Self {
			backends: Backends::all(),
			features: Features::empty(),
			optional_features: Features::empty(),
			selection: None,
			environment_overrides: false,
		}
	}
	/// Lets wgpu's environment variables (`WGPU_BACKEND`, `WGPU_DX12_COMPILER`, `WGPU_DEBUG`, `WGPU_VALIDATION`, and the like) override the instance's settings.
	pub fn with_environment_overrides(mut self) -> Self {
		self.environment_overrides = true;
		self
	}
	pub fn with_backends(mut self, backends: Backends) -> Self {
		self.backends = backends;
		self
	}
	pub fn with_features(mut self, features: Features) -> Self {
		self.features = features;
		self
	}
	pub fn with_optional_features(mut self, features: Features) -> Self {
		self.optional_features = features;
		self
	}
	pub fn with_selection(mut self, index: usize) -> Self {
		self.selection = Some(index);
		self
	}
}
#[cfg(not(target_family = "wasm"))]
impl ContextBuilder {
	pub async fn build(self) -> Option<Context> {
		let instance = self.build_instance();
		let mut adapters = enumerate_sorted(&instance, self.backends).await;

		if let Some(index) = self.selection
			&& index < adapters.len()
		{
			let selected_adapter = adapters.remove(index);
			adapters.insert(0, selected_adapter);
		}

		for adapter in adapters {
			if let Some((device, queue)) = self.request_device(&adapter, adapter.features()).await {
				return Some(Context { device, queue, adapter, instance });
			}
		}
		None
	}
	pub async fn available_adapters_fmt(&self) -> impl std::fmt::Display {
		let instance = self.build_instance();
		fmt::AvailableAdaptersFormatter(enumerate_sorted(&instance, self.backends).await)
	}
}
#[cfg(target_family = "wasm")]
impl ContextBuilder {
	pub async fn build_with_surface(self, target: impl Into<wgpu::SurfaceTarget<'static>>) -> Option<(Context, Surface)> {
		let target = target.into();
		let canvas = match &target {
			wgpu::SurfaceTarget::Canvas(canvas) => Some(canvas.clone()),
			_ => None,
		};

		let instance = self.build_instance();
		let surface = instance.create_surface(target).ok()?;
		let request_adapter_options = wgpu::RequestAdapterOptions {
			power_preference: wgpu::PowerPreference::HighPerformance,
			compatible_surface: Some(surface.raw()),
			force_fallback_adapter: false,
			apply_limit_buckets: false,
		};
		let adapter = instance.request_adapter(&request_adapter_options).await.ok()?;

		// WebGL2 offers the 16-bit unorm formats wherever `EXT_texture_norm16` is supported, but they only work once it's enabled
		let mut available_features = adapter.features();
		if !canvas.is_some_and(|canvas| enable_texture_norm16(&canvas)) {
			available_features.remove(Features::TEXTURE_FORMAT_16BIT_NORM);
		}
		let (device, queue) = self.request_device(&adapter, available_features).await?;

		Some((Context { device, queue, adapter, instance }, surface))
	}
}

/// Enables `EXT_texture_norm16` on the WebGL2 context wgpu made for the `canvas`, returning whether it did.
#[cfg(target_family = "wasm")]
fn enable_texture_norm16(canvas: &web_sys::HtmlCanvasElement) -> bool {
	use web_sys::wasm_bindgen::JsCast;

	// TODO: Remove this check once Firefox 162 is out, since 159 to 161 offer the extension but can't filter its formats, which then read as black (Bugzilla 2086168)
	let user_agent = web_sys::window().and_then(|window| window.navigator().user_agent().ok()).unwrap_or_default();
	if firefox_version(&user_agent).is_some_and(|version| (159..=161).contains(&version)) {
		return false;
	}

	// Asking the canvas for its context again returns the one wgpu made
	let Ok(Some(context)) = canvas.get_context("webgl2") else { return false };
	let Ok(context) = context.dyn_into::<web_sys::WebGl2RenderingContext>() else { return false };
	context.get_extension("EXT_texture_norm16").is_ok_and(|extension| extension.is_some())
}

// TODO: Remove along with the Firefox 159 to 161 check in `enable_texture_norm16` once Firefox 162 is out
/// The major version of Firefox that a browser's `user_agent` string names, or `None` for any other browser.
#[cfg(any(target_family = "wasm", test))]
fn firefox_version(user_agent: &str) -> Option<u32> {
	user_agent.split("Firefox/").nth(1)?.split('.').next()?.parse().ok()
}

impl ContextBuilder {
	fn build_instance(&self) -> Instance {
		let descriptor = wgpu::InstanceDescriptor {
			backends: self.backends,
			..wgpu::InstanceDescriptor::new_without_display_handle()
		};

		Instance::new(wgpu::Instance::new(if self.environment_overrides { descriptor.with_env() } else { descriptor }))
	}
	/// Requests a device with the builder's features, plus whichever optional ones are among the `available_features`.
	async fn request_device(&self, adapter: &Adapter, available_features: Features) -> Option<(Device, Queue)> {
		let optional_features = self.optional_features | RENDERER_OPTIONAL_FEATURES;
		let device_descriptor = wgpu::DeviceDescriptor {
			label: None,
			required_features: self.features | (optional_features & available_features),
			required_limits: adapter.limits(),
			memory_hints: Default::default(),
			trace: wgpu::Trace::Off,
			experimental_features: Default::default(),
		};
		adapter.request_device(&device_descriptor).await.ok()
	}
}
#[cfg(not(target_family = "wasm"))]
async fn enumerate_sorted(instance: &Instance, backends: Backends) -> Vec<Adapter> {
	let mut adapters = instance.enumerate_adapters(backends).await;
	adapters.sort_by_key(adapter_priority);
	adapters
}
#[cfg(not(target_family = "wasm"))]
fn adapter_priority(adapter: &Adapter) -> (u8, u8) {
	let info = adapter.get_info();
	let backend = if cfg!(target_os = "linux") {
		match info.backend {
			wgpu::Backend::Vulkan => 0,
			_ => 1,
		}
	} else if cfg!(target_os = "windows") {
		match info.backend {
			wgpu::Backend::Dx12 => 0,
			_ => 1,
		}
	} else {
		0
	};
	let device_type = match info.device_type {
		wgpu::DeviceType::DiscreteGpu => 0,
		wgpu::DeviceType::IntegratedGpu => 1,
		wgpu::DeviceType::VirtualGpu => 2,
		wgpu::DeviceType::Cpu => 3,
		wgpu::DeviceType::Other => 4,
	};
	(backend, device_type)
}
#[cfg(not(target_family = "wasm"))]
mod fmt {
	use super::*;

	pub(super) struct AvailableAdaptersFormatter(pub(super) Vec<Adapter>);
	impl std::fmt::Display for AvailableAdaptersFormatter {
		fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
			for (i, adapter) in self.0.iter().enumerate() {
				let info = adapter.get_info();
				writeln!(
					f,
					"[{}] {:?} {:?} (Name: {}, Driver: {}, Device: {})",
					i, info.backend, info.device_type, info.name, info.driver, info.device,
				)?;
			}
			Ok(())
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	// TODO: Remove along with `firefox_version` once Firefox 162 is out
	#[test]
	fn firefox_versions_are_read_from_user_agents() {
		assert_eq!(firefox_version("Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:160.0) Gecko/20100101 Firefox/160.0"), Some(160));
		assert_eq!(firefox_version("Mozilla/5.0 (X11; Linux x86_64; rv:163.0) Gecko/20100101 Firefox/163.0a1"), Some(163));
		assert_eq!(
			firefox_version("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36"),
			None
		);

		// Firefox on iOS draws with Safari's engine and names itself differently
		assert_eq!(
			firefox_version("Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) FxiOS/160.0 Mobile/15E148 Safari/605.1.15"),
			None
		);
	}
}
