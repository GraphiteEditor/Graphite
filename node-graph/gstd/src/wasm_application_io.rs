pub use graph_craft::document::value::RenderOutput;
pub use graph_craft::wasm_application_io::*;
#[cfg(target_arch = "wasm32")]
use graphene_core::application_io::SurfaceHandle;
use graphene_core::application_io::{ApplicationIo, ExportFormat, RenderConfig};
#[cfg(target_arch = "wasm32")]
use graphene_core::raster::bbox::Bbox;
use graphene_core::raster::Image;
use graphene_core::raster::ImageFrame;
use graphene_core::renderer::{format_transform_matrix, GraphicElementRendered, ImageRenderMode, RenderParams, RenderSvgSegmentList, SvgRender};
use graphene_core::transform::Footprint;
use graphene_core::Node;
use graphene_core::{Color, WasmNotSend};

#[cfg(target_arch = "wasm32")]
use base64::Engine;
#[cfg(target_arch = "wasm32")]
use glam::DAffine2;
use std::sync::Arc;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::Clamped;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

pub struct CreateSurfaceNode {}

#[node_macro::node_fn(CreateSurfaceNode)]
async fn create_surface_node<'a: 'input>(editor: &'a WasmEditorApi) -> Arc<WasmSurfaceHandle> {
	Arc::new(editor.application_io.as_ref().unwrap().create_surface())
}

#[cfg(target_arch = "wasm32")]
pub struct DrawImageFrameNode<Surface> {
	surface_handle: Surface,
}

#[node_macro::node_fn(DrawImageFrameNode)]
#[cfg(target_arch = "wasm32")]
async fn draw_image_frame_node<'a: 'input>(
	image: ImageFrame<graphene_core::raster::SRGBA8>,
	surface_handle: Arc<WasmSurfaceHandle>,
) -> graphene_core::application_io::SurfaceHandleFrame<HtmlCanvasElement> {
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
	graphene_core::application_io::SurfaceHandleFrame {
		surface_handle,
		transform: image.transform,
	}
}

pub struct LoadResourceNode<Url> {
	url: Url,
}

#[node_macro::node_fn(LoadResourceNode)]
async fn load_resource_node<'a: 'input>(editor: &'a WasmEditorApi, url: String) -> Arc<[u8]> {
	editor.application_io.as_ref().unwrap().load_resource(url).unwrap().await.unwrap()
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
			..Default::default()
		},
		..Default::default()
	};
	image
}

fn render_svg(data: impl GraphicElementRendered, mut render: SvgRender, render_params: RenderParams, footprint: Footprint) -> RenderOutput {
	if !data.contains_artboard() && !render_params.hide_artboards {
		render.leaf_tag("rect", |attributes| {
			attributes.push("x", "0");
			attributes.push("y", "0");
			attributes.push("width", footprint.resolution.x.to_string());
			attributes.push("height", footprint.resolution.y.to_string());
			attributes.push("transform", format_transform_matrix(footprint.transform.inverse()));
			attributes.push("fill", "white");
		});
	}

	data.render_svg(&mut render, &render_params);
	render.wrap_with_transform(footprint.transform, Some(footprint.resolution.as_dvec2()));

	RenderOutput::Svg(render.svg.to_svg_string())
}

#[cfg(feature = "vello")]
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
async fn render_canvas(render_config: RenderConfig, data: impl GraphicElementRendered, editor: &WasmEditorApi, surface_handle: wgpu_executor::WgpuSurface) -> RenderOutput {
	if let Some(exec) = editor.application_io.as_ref().unwrap().gpu_executor() {
		use vello::*;

		let footprint = render_config.viewport;

		let mut scene = Scene::new();
		let mut child = Scene::new();

		data.render_to_vello(&mut child, glam::DAffine2::IDENTITY);

		// TODO: Instead of applying the transform here, pass the transform during the translation to avoid the O(Nr cost
		scene.append(&child, Some(kurbo::Affine::new(footprint.transform.to_cols_array())));

		exec.render_vello_scene(&scene, &surface_handle, footprint.resolution.x, footprint.resolution.y)
			.await
			.expect("Failed to render Vello scene");
	} else {
		unreachable!("Attempted to render with Vello when no GPU executor is available");
	}
	let frame = graphene_core::application_io::SurfaceHandleFrame {
		surface_handle,
		transform: glam::DAffine2::IDENTITY,
	};
	RenderOutput::CanvasFrame(frame.into())
}

#[cfg(target_arch = "wasm32")]
pub struct RasterizeNode<Footprint, Surface> {
	footprint: Footprint,
	surface_handle: Surface,
}

#[node_macro::node_fn(RasterizeNode)]
#[cfg(target_arch = "wasm32")]
async fn rasterize<_T: GraphicElementRendered + graphene_core::transform::TransformMut + WasmNotSend>(
	mut data: _T,
	footprint: Footprint,
	surface_handle: Arc<SurfaceHandle<HtmlCanvasElement>>,
) -> ImageFrame<Color> {
	let mut render = SvgRender::new();

	if footprint.transform.matrix2.determinant() == 0. {
		log::trace!("Invalid footprint received for rasterization");
		return ImageFrame::default();
	}
	let aabb = Bbox::from_transform(footprint.transform).to_axis_aligned_bbox();
	let size = aabb.size();
	let resolution = footprint.resolution;
	let render_params = RenderParams {
		culling_bounds: None,
		..Default::default()
	};

	*data.transform_mut() = DAffine2::from_translation(-aabb.start) * data.transform();
	data.render_svg(&mut render, &render_params);
	render.format_svg(glam::DVec2::ZERO, size);
	let svg_string = render.svg.to_svg_string();

	let canvas = &surface_handle.surface;
	canvas.set_width(resolution.x);
	canvas.set_height(resolution.y);

	let context = canvas.get_context("2d").unwrap().unwrap().dyn_into::<CanvasRenderingContext2d>().unwrap();

	let preamble = "data:image/svg+xml;base64,";
	let mut base64_string = String::with_capacity(preamble.len() + svg_string.len() * 4);
	base64_string.push_str(preamble);
	base64::engine::general_purpose::STANDARD.encode_string(svg_string, &mut base64_string);

	let image_data = web_sys::HtmlImageElement::new().unwrap();
	image_data.set_src(base64_string.as_str());
	wasm_bindgen_futures::JsFuture::from(image_data.decode()).await.unwrap();
	context
		.draw_image_with_html_image_element_and_dw_and_dh(&image_data, 0., 0., resolution.x as f64, resolution.y as f64)
		.unwrap();

	let rasterized = context.get_image_data(0., 0., resolution.x as f64, resolution.y as f64).unwrap();

	let image = Image::from_image_data(&rasterized.data().0, resolution.x as u32, resolution.y as u32);
	ImageFrame {
		image,
		transform: footprint.transform,
		..Default::default()
	}
}

pub struct RenderNode<EditorApi, Data, Surface> {
	editor_api: EditorApi,
	data: Data,
	_surface_handle: Surface,
}

#[node_macro::node_fn(RenderNode)]
async fn render_node<'a: 'input, T: 'input + GraphicElementRendered + WasmNotSend>(
	render_config: RenderConfig,
	editor_api: &'a WasmEditorApi,
	data: impl Node<Footprint, Output = T>,
	_surface_handle: impl Node<Footprint, Output = Option<wgpu_executor::WgpuSurface>>,
) -> RenderOutput {
	let footprint = render_config.viewport;

	let RenderConfig { hide_artboards, for_export, .. } = render_config;
	let render_params = RenderParams::new(render_config.view_mode, ImageRenderMode::Base64, None, false, hide_artboards, for_export);

	let data = self.data.eval(footprint).await;
	#[cfg(all(feature = "vello", target_arch = "wasm32"))]
	let surface_handle = self._surface_handle.eval(footprint).await;
	let use_vello = editor_api.editor_preferences.use_vello();
	#[cfg(all(feature = "vello", target_arch = "wasm32"))]
	let use_vello = use_vello && surface_handle.is_some();

	let output_format = render_config.export_format;
	match output_format {
		ExportFormat::Svg => render_svg(data, SvgRender::new(), render_params, footprint),
		ExportFormat::Canvas => {
			if use_vello && editor_api.application_io.as_ref().unwrap().gpu_executor().is_some() {
				#[cfg(all(feature = "vello", target_arch = "wasm32"))]
				return render_canvas(render_config, data, editor_api, surface_handle.unwrap()).await;
				#[cfg(not(all(feature = "vello", target_arch = "wasm32")))]
				render_svg(data, SvgRender::new(), render_params, footprint)
			} else {
				render_svg(data, SvgRender::new(), render_params, footprint)
			}
		}
		_ => todo!("Non-SVG render output for {output_format:?}"),
	}
}
