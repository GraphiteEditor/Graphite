use std::cell::RefCell;

use core::future::Future;
use dyn_any::StaticType;
use graphene_core::application_io::{ApplicationError, ApplicationIo, ExportFormat, ResourceFuture, SurfaceHandle, SurfaceHandleFrame, SurfaceId};
use graphene_core::raster::Image;
use graphene_core::renderer::{GraphicElementRendered, RenderParams, SvgRender};
use graphene_core::transform::Footprint;
use graphene_core::vector::style::ViewMode;
use graphene_core::{
	raster::{color::SRGBA8, ImageFrame},
	Node,
};
use graphene_core::{Color, GraphicGroup};
#[cfg(target_arch = "wasm32")]
use js_sys::{Object, Reflect};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
#[cfg(feature = "tokio")]
use tokio::io::AsyncReadExt;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsValue;
use wasm_bindgen::{Clamped, JsCast};
#[cfg(target_arch = "wasm32")]
use web_sys::window;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};
#[cfg(feature = "wgpu")]
use wgpu_executor::WgpuExecutor;

use base64::Engine;
use glam::DAffine2;

pub struct Canvas(CanvasRenderingContext2d);

#[derive(Debug, Default)]
pub struct WasmApplicationIo {
	#[cfg(target_arch = "wasm32")]
	ids: RefCell<u64>,
	#[cfg(feature = "wgpu")]
	pub(crate) gpu_executor: Option<WgpuExecutor>,
	#[cfg(not(target_arch = "wasm32"))]
	windows: RefCell<Vec<Arc<winit::window::Window>>>,
	pub resources: HashMap<String, Arc<[u8]>>,
}

impl WasmApplicationIo {
	pub async fn new() -> Self {
		#[cfg(all(feature = "wgpu", target_arch = "wasm32"))]
		let executor = if let Some(gpu) = web_sys::window().map(|w| w.navigator().gpu()) {
			let request_adapter = || {
				let request_adapter = js_sys::Reflect::get(&gpu, &wasm_bindgen::JsValue::from_str("requestAdapter")).ok()?;
				let function = request_adapter.dyn_ref::<js_sys::Function>()?;
				Some(function.call0(&gpu).ok())
			};
			let result = request_adapter();
			match result {
				None => None,
				Some(_) => WgpuExecutor::new().await,
			}
		} else {
			None
		};
		#[cfg(all(feature = "wgpu", not(target_arch = "wasm32")))]
		let executor = WgpuExecutor::new().await;
		let mut io = Self {
			#[cfg(target_arch = "wasm32")]
			ids: RefCell::new(0),
			#[cfg(feature = "wgpu")]
			gpu_executor: executor,
			#[cfg(not(target_arch = "wasm32"))]
			windows: RefCell::new(Vec::new()),
			resources: HashMap::new(),
		};
		io.resources.insert("null".to_string(), Arc::from(include_bytes!("null.png").to_vec()));
		io
	}
}

unsafe impl StaticType for WasmApplicationIo {
	type Static = WasmApplicationIo;
}

impl<'a> From<WasmEditorApi<'a>> for &'a WasmApplicationIo {
	fn from(editor_api: WasmEditorApi<'a>) -> Self {
		editor_api.application_io
	}
}
#[cfg(feature = "wgpu")]
impl<'a> From<&'a WasmApplicationIo> for &'a WgpuExecutor {
	fn from(app_io: &'a WasmApplicationIo) -> Self {
		app_io.gpu_executor.as_ref().unwrap()
	}
}

pub type WasmEditorApi<'a> = graphene_core::application_io::EditorApi<'a, WasmApplicationIo>;

impl ApplicationIo for WasmApplicationIo {
	#[cfg(target_arch = "wasm32")]
	type Surface = HtmlCanvasElement;
	#[cfg(not(target_arch = "wasm32"))]
	type Surface = Arc<winit::window::Window>;
	#[cfg(feature = "wgpu")]
	type Executor = WgpuExecutor;
	#[cfg(not(feature = "wgpu"))]
	type Executor = ();

	#[cfg(target_arch = "wasm32")]
	fn create_surface(&self) -> SurfaceHandle<Self::Surface> {
		let wrapper = || {
			let document = window().expect("should have a window in this context").document().expect("window should have a document");

			let canvas: HtmlCanvasElement = document.create_element("canvas")?.dyn_into::<HtmlCanvasElement>()?;
			let mut guard = self.ids.borrow_mut();
			let id = SurfaceId(*guard);
			*guard += 1;
			// store the canvas in the global scope so it doesn't get garbage collected
			let window = window().expect("should have a window in this context");
			let window = Object::from(window);

			let image_canvases_key = JsValue::from_str("imageCanvases");

			let mut canvases = Reflect::get(&window, &image_canvases_key);
			if canvases.is_err() {
				Reflect::set(&JsValue::from(web_sys::window().unwrap()), &image_canvases_key, &Object::new()).unwrap();
				canvases = Reflect::get(&window, &image_canvases_key);
			}

			// Convert key and value to JsValue
			let js_key = JsValue::from_str(format!("canvas{}", id.0).as_str());
			let js_value = JsValue::from(canvas.clone());

			let canvases = Object::from(canvases.unwrap());

			// Use Reflect API to set property
			Reflect::set(&canvases, &js_key, &js_value)?;
			Ok::<_, JsValue>(SurfaceHandle { surface_id: id, surface: canvas })
		};

		wrapper().expect("should be able to set canvas in global scope")
	}
	#[cfg(not(target_arch = "wasm32"))]
	fn create_surface(&self) -> SurfaceHandle<Self::Surface> {
		#[cfg(feature = "wayland")]
		use winit::platform::wayland::EventLoopBuilderExtWayland;

		#[cfg(feature = "wayland")]
		let event_loop = winit::event_loop::EventLoopBuilder::new().with_any_thread(true).build();
		#[cfg(not(feature = "wayland"))]
		let event_loop = winit::event_loop::EventLoop::new();
		let window = winit::window::WindowBuilder::new()
			.with_title("Graphite")
			.with_inner_size(winit::dpi::PhysicalSize::new(800, 600))
			.build(&event_loop)
			.unwrap();
		let window = Arc::new(window);
		self.windows.borrow_mut().push(window.clone());
		SurfaceHandle {
			surface_id: SurfaceId(window.id().into()),
			surface: window,
		}
	}

	#[cfg(target_arch = "wasm32")]
	fn destroy_surface(&self, surface_id: SurfaceId) {
		let window = window().expect("should have a window in this context");
		let window = Object::from(window);

		let image_canvases_key = JsValue::from_str("imageCanvases");

		let wrapper = || {
			if let Ok(canvases) = Reflect::get(&window, &image_canvases_key) {
				// Convert key and value to JsValue
				let js_key = JsValue::from_str(format!("canvas{}", surface_id.0).as_str());

				// Use Reflect API to set property
				Reflect::delete_property(&canvases.into(), &js_key)?;
			}
			Ok::<_, JsValue>(())
		};

		wrapper().expect("should be able to set canvas in global scope")
	}

	#[cfg(not(target_arch = "wasm32"))]
	fn destroy_surface(&self, _surface_id: SurfaceId) {}

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
				}) as Pin<Box<dyn Future<Output = Result<Arc<[u8]>, _>>>>)
			}
			"http" | "https" => {
				let url = url.to_string();
				Ok(Box::pin(async move {
					let client = reqwest::Client::new();
					let response = client.get(url).send().await.map_err(|_| ApplicationError::NotFound)?;
					let data = response.bytes().await.map_err(|_| ApplicationError::NotFound)?;
					Ok(Arc::from(data.to_vec()))
				}) as Pin<Box<dyn Future<Output = Result<Arc<[u8]>, _>>>>)
			}
			"graphite" => {
				let path = url.path();
				let path = path.to_owned();
				log::trace!("Loading local resource: {path}");
				let data = self.resources.get(&path).ok_or(ApplicationError::NotFound)?.clone();
				Ok(Box::pin(async move { Ok(data.clone()) }) as Pin<Box<dyn Future<Output = Result<Arc<[u8]>, _>>>>)
			}
			_ => Err(ApplicationError::NotFound),
		}
	}
}

pub type WasmSurfaceHandle = SurfaceHandle<HtmlCanvasElement>;
pub type WasmSurfaceHandleFrame = SurfaceHandleFrame<HtmlCanvasElement>;

pub struct CreateSurfaceNode {}

#[node_macro::node_fn(CreateSurfaceNode)]
async fn create_surface_node<'a: 'input>(editor: WasmEditorApi<'a>) -> Arc<SurfaceHandle<<WasmApplicationIo as ApplicationIo>::Surface>> {
	editor.application_io.create_surface().into()
}

pub struct DrawImageFrameNode<Surface> {
	surface_handle: Surface,
}

#[node_macro::node_fn(DrawImageFrameNode)]
async fn draw_image_frame_node<'a: 'input>(image: ImageFrame<SRGBA8>, surface_handle: Arc<SurfaceHandle<HtmlCanvasElement>>) -> SurfaceHandleFrame<HtmlCanvasElement> {
	let image_data = image.image.data;
	let array: Clamped<&[u8]> = Clamped(bytemuck::cast_slice(image_data.as_slice()));
	if image.image.width > 0 && image.image.height > 0 {
		let canvas = &surface_handle.surface;
		canvas.set_width(image.image.width);
		canvas.set_height(image.image.height);
		// TODO: replace "2d" with "bitmaprenderer" once we switch to ImageBitmap (lives on gpu) from ImageData (lives on cpu)
		let context = canvas.get_context("2d").unwrap().unwrap().dyn_into::<CanvasRenderingContext2d>().unwrap();
		let image_data = web_sys::ImageData::new_with_u8_clamped_array_and_sh(array, image.image.width, image.image.height).expect("Failed to construct ImageData");
		context.put_image_data(&image_data, 0.0, 0.0).unwrap();
	}
	SurfaceHandleFrame {
		surface_handle,
		transform: image.transform,
	}
}

pub struct LoadResourceNode<Url> {
	url: Url,
}

#[node_macro::node_fn(LoadResourceNode)]
async fn load_resource_node<'a: 'input>(editor: WasmEditorApi<'a>, url: String) -> Arc<[u8]> {
	editor.application_io.load_resource(url).unwrap().await.unwrap()
}

pub struct DecodeImageNode;

#[node_macro::node_fn(DecodeImageNode)]
fn decode_image_node<'a: 'input>(data: Arc<[u8]>) -> ImageFrame<Color> {
	let image = image::load_from_memory(data.as_ref()).expect("Failed to decode image");
	let image = image.to_rgba32f();
	let image = ImageFrame {
		image: Image {
			data: image.chunks(4).map(|pixel| Color::from_unassociated_alpha(pixel[0], pixel[1], pixel[2], pixel[3])).collect(),
			width: image.width(),
			height: image.height(),
		},
		transform: glam::DAffine2::IDENTITY,
	};
	image
}
pub use graph_craft::document::value::RenderOutput;

pub struct RenderNode<Data, Surface, Parameter> {
	data: Data,
	surface_handle: Surface,
	parameter: PhantomData<Parameter>,
}

fn render_svg(data: impl GraphicElementRendered, mut render: SvgRender, render_params: RenderParams, footprint: Footprint) -> RenderOutput {
	data.render_svg(&mut render, &render_params);
	render.wrap_with_transform(footprint.transform);
	RenderOutput::Svg(render.svg.to_string())
}

#[cfg(any(feature = "resvg", feature = "vello"))]
fn render_canvas(
	data: impl GraphicElementRendered,
	mut render: SvgRender,
	render_params: RenderParams,
	footprint: Footprint,
	editor: WasmEditorApi<'_>,
	surface_handle: Arc<SurfaceHandle<HtmlCanvasElement>>,
) -> RenderOutput {
	let resolution = footprint.resolution;
	data.render_svg(&mut render, &render_params);
	// TODO: reenable once we switch to full node graph
	let min = footprint.transform.inverse().transform_point2((0., 0.).into());
	let max = footprint.transform.inverse().transform_point2(resolution.as_dvec2());
	render.format_svg(min, max);
	let string = render.svg.to_string();
	let array = string.as_bytes();
	let canvas = &surface_handle.surface;
	canvas.set_width(resolution.x);
	canvas.set_height(resolution.y);
	let usvg_tree = data.to_usvg_tree(resolution, [min, max]);

	if let Some(exec) = editor.application_io.gpu_executor() {
		todo!()
	} else {
		let rtree = resvg::Tree::from_usvg(&usvg_tree);

		let pixmap_size = rtree.size.to_int_size();
		let mut pixmap = resvg::tiny_skia::Pixmap::new(pixmap_size.width(), pixmap_size.height()).unwrap();
		rtree.render(resvg::tiny_skia::Transform::default(), &mut pixmap.as_mut());
		let array: Clamped<&[u8]> = Clamped(pixmap.data());
		let context = canvas.get_context("2d").unwrap().unwrap().dyn_into::<CanvasRenderingContext2d>().unwrap();
		let image_data = web_sys::ImageData::new_with_u8_clamped_array_and_sh(array, pixmap_size.width(), pixmap_size.height()).expect("Failed to construct ImageData");
		context.put_image_data(&image_data, 0.0, 0.0).unwrap();
	}
	/*
	let preamble = "data:image/svg+xml;base64,";
	let mut base64_string = String::with_capacity(preamble.len() + array.len() * 4);
	base64_string.push_str(preamble);
	base64::engine::general_purpose::STANDARD.encode_string(array, &mut base64_string);

	let image_data = web_sys::HtmlImageElement::new().unwrap();
	image_data.set_src(base64_string.as_str());
	wasm_bindgen_futures::JsFuture::from(image_data.decode()).await.unwrap();
	context.draw_image_with_html_image_element(&image_data, 0.0, 0.0).unwrap();
	*/
	let frame = SurfaceHandleFrame {
		surface_handle,
		transform: DAffine2::IDENTITY,
	};
	RenderOutput::CanvasFrame(frame.into())
}

// Render with the data node taking in Footprint.
impl<'input, 'a: 'input, T: 'input + GraphicElementRendered, F: 'input + Future<Output = T>, Data: 'input, Surface: 'input, SurfaceFuture: 'input> Node<'input, WasmEditorApi<'a>>
	for RenderNode<Data, Surface, Footprint>
where
	Data: Node<'input, Footprint, Output = F>,
	Surface: Node<'input, (), Output = SurfaceFuture>,
	SurfaceFuture: core::future::Future<Output = Arc<SurfaceHandle<HtmlCanvasElement>>>,
{
	type Output = core::pin::Pin<Box<dyn core::future::Future<Output = RenderOutput> + 'input>>;

	#[inline]
	fn eval(&'input self, editor: WasmEditorApi<'a>) -> Self::Output {
		Box::pin(async move {
			let footprint = editor.render_config.viewport;
			let render_params = RenderParams::new(ViewMode::Normal, graphene_core::renderer::ImageRenderMode::Base64, None, false);

			let output_format = editor.render_config.export_format;
			match output_format {
				ExportFormat::Svg => render_svg(self.data.eval(footprint).await, SvgRender::new(), render_params, footprint),
				#[cfg(any(feature = "resvg", feature = "vello"))]
				ExportFormat::Canvas => render_canvas(self.data.eval(footprint).await, SvgRender::new(), render_params, footprint, editor, self.surface_handle.eval(()).await),
				_ => todo!("Non-SVG render output for {output_format:?}"),
			}
		})
	}
}

// Render with the data node taking in ().
impl<'input, 'a: 'input, T: 'input + GraphicElementRendered, F: 'input + Future<Output = T>, Data: 'input, Surface: 'input, SurfaceFuture: 'input> Node<'input, WasmEditorApi<'a>>
	for RenderNode<Data, Surface, ()>
where
	Data: Node<'input, (), Output = F>,
	Surface: Node<'input, (), Output = SurfaceFuture>,
	SurfaceFuture: core::future::Future<Output = Arc<SurfaceHandle<HtmlCanvasElement>>>,
{
	type Output = core::pin::Pin<Box<dyn core::future::Future<Output = RenderOutput> + 'input>>;
	#[inline]
	fn eval(&'input self, editor: WasmEditorApi<'a>) -> Self::Output {
		Box::pin(async move {
			use graphene_core::renderer::ImageRenderMode;

			let footprint = editor.render_config.viewport;
			let render_params = RenderParams::new(ViewMode::Normal, ImageRenderMode::Base64, None, false);

			let output_format = editor.render_config.export_format;
			match output_format {
				ExportFormat::Svg => render_svg(self.data.eval(()).await, SvgRender::new(), render_params, footprint),
				#[cfg(any(feature = "resvg", feature = "vello"))]
				ExportFormat::Canvas => render_canvas(self.data.eval(()).await, SvgRender::new(), render_params, footprint, editor, self.surface_handle.eval(()).await),
				_ => todo!("Non-SVG render output for {output_format:?}"),
			}
		})
	}
}
#[automatically_derived]
impl<Data, Surface, Parameter> RenderNode<Data, Surface, Parameter> {
	pub const fn new(data: Data, surface_handle: Surface) -> Self {
		Self {
			data,
			surface_handle,
			parameter: PhantomData,
		}
	}
}
