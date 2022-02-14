use glam::Vec2;
use graphene::layers::style::PathStyle;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Document, HtmlCanvasElement, WebGl2RenderingContext, WebGlProgram, WebGlShader};

#[derive(Clone)]
pub struct RenderingContext {
	document: Document,
	canvas: HtmlCanvasElement,
	context: WebGl2RenderingContext,
	scale: f64,
	program: WebGlProgram,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
#[allow(unused_variables)]
pub struct VertexData {
	line_start: [f32; 2],
	line_end: [f32; 2],
	color: [f32; 4],
	width: f32,
	zindex: f32,
	transform: [f32; 6],
}

impl VertexData {
	pub fn new(line_start: Vec2, line_end: Vec2, zindex: f32, width: f32, color: editor::Color) -> Self {
		let a = line_start;
		let b = line_end;

		let v = (a - b).normalize_or_zero() * std::f32::consts::SQRT_2 * width * 0.5;
		let pv = v.perp();
		let a1 = a + v + pv;
		let a2 = a + v - pv;
		let b1 = b - v - pv;
		let b2 = b - v + pv;

		let scalex = a1.distance(b2) / 2.;
		let scaley = a1.distance(a2) / 2.;
		let angle = v.angle_between((1., 0.).into());
		let matrix = glam::Affine2::from_scale_angle_translation((scalex, scaley).into(), angle, a.lerp(b, 0.5));

		Self {
			line_start: line_start.into(),
			line_end: line_start.into(),
			color: [color.r(), color.g(), color.b(), color.a()],
			zindex,
			width,
			transform: matrix.to_cols_array(),
		}
	}
}

impl RenderingContext {
	pub fn new() -> Result<Self, JsValue> {
		let document = web_sys::window().unwrap().document().unwrap();
		let scale = web_sys::window().unwrap().device_pixel_ratio();
		let canvas = document.query_selector(".rendering-canvas").unwrap().unwrap();
		let canvas: web_sys::HtmlCanvasElement = canvas.dyn_into::<web_sys::HtmlCanvasElement>()?;
		let map = js_sys::Map::new();
		map.set(&JsValue::from_str("premultipliedalpha"), &JsValue::from_str("false"));

		//.get_context_with_context_options("experimental-webgl", map.as_ref())
		let context = canvas.get_context("webgl2").unwrap().unwrap().dyn_into::<WebGl2RenderingContext>()?;
		context.blend_func_separate(
			WebGl2RenderingContext::SRC_ALPHA,
			WebGl2RenderingContext::ONE_MINUS_SRC_ALPHA,
			WebGl2RenderingContext::ZERO,
			WebGl2RenderingContext::ONE,
		);

		let vert_shader = compile_shader(&context, WebGl2RenderingContext::VERTEX_SHADER, include_str!("../shaders/shader.vert"))?;

		let frag_shader = compile_shader(&context, WebGl2RenderingContext::FRAGMENT_SHADER, include_str!("../shaders/shader.frag"))?;
		let program = link_program(&context, &vert_shader, &frag_shader)?;
		context.use_program(Some(&program));
		context.viewport(0, 0, canvas.width() as i32, canvas.height() as i32);
		Ok(Self {
			document,
			canvas,
			context,
			scale,
			program,
		})
	}
	pub fn draw_paths(&mut self, lines: impl Iterator<Item = (Vec<(Vec2, Vec2)>, PathStyle, u32)>) {
		let mut buffer = Vec::new();
		for (segments, style, depth) in lines {
			let stroke = style.stroke().unwrap();
			for (line_start, line_end) in segments {
				buffer.push(VertexData::new(line_start, line_end, depth as f32 / 100., stroke.width(), stroke.color()))
			}
		}

		self.draw(buffer);
		//self.draw_lines(&[(500.7, 500.7, 3100.7, 2700.7)]);
	}

	pub fn draw(&mut self, vertex_data: Vec<VertexData>) -> Result<(), JsValue> {
		self.context.viewport(0, 0, self.canvas.width() as i32, self.canvas.height() as i32);
		//let (vertex_data, index_data) = create_vertices(&[(-0.5, -0.5, 0.5, 0.5), (-0.5, 0.5, 0.5, -0.5), (-0.5, -0.5, 0.5, -0.5), (-0.5, 0.5, 0.5, 0.5)], 0.15);
		let float_size = std::mem::size_of::<f32>() as i32;
		let vertex_size = std::mem::size_of::<VertexData>() as i32;

		let vertices: &[f32] = unsafe { std::slice::from_raw_parts(vertex_data.as_ptr() as *const f32, vertex_data.len() * vertex_size as usize / float_size as usize) };

		let buffer = self.context.create_buffer().ok_or("Failed to create buffer")?;
		self.context.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&buffer));

		// Note that `Float32Array::view` is somewhat dangerous (hence the
		// `unsafe`!). This is creating a raw view into our module's
		// `WebAssembly.Memory` buffer, but if we allocate more pages for ourself
		// (aka do a memory allocation in Rust) it'll cause the buffer to change,
		// causing the `Float32Array` to be invalid.
		//
		// As a result, after `Float32Array::view` we have to be very careful not to
		// do any memory allocations before it's dropped.
		//let vertices = std::mem::transmute(&vertices[..]);
		//log::debug!("vertices: {vertices:?}");

		let positions_array_buf_view = js_sys::Float32Array::new_with_length(vertices.len() as u32);
		positions_array_buf_view.copy_from(vertices);

		let matrix_location = self.context.get_uniform_location(&self.program, "matrix");
		let transform = glam::Affine2::from_scale(20. * Vec2::new(self.canvas.width() as f32, self.canvas.height() as f32).recip());
		let transform = glam::Affine2::from_scale(self.scale as f32 * Vec2::new(1., -1.)) * transform;
		let transform = glam::Affine2::from_translation(Vec2::new(-1., 1.)) * transform;
		self.context.uniform_matrix2x3fv_with_f32_array(matrix_location.as_ref(), false, &transform.to_cols_array());

		self.context
			.buffer_data_with_array_buffer_view(WebGl2RenderingContext::ARRAY_BUFFER, &positions_array_buf_view, WebGl2RenderingContext::DYNAMIC_DRAW);

		let vao = self.context.create_vertex_array().ok_or("Could not create vertex array object")?;
		self.context.bind_vertex_array(Some(&vao));

		//log::debug!("positon location: {position_attribute_location:?}");
		//log::debug!("line location: {line_attribute_location:?}");

		let line_start_attribute_location = self.context.get_attrib_location(&self.program, "line_segment_start");
		let line_end_attribute_location = self.context.get_attrib_location(&self.program, "line_segment_end");
		let color_attribute_location = self.context.get_attrib_location(&self.program, "line_color");
		let offset_matrix_attribute_location = self.context.get_attrib_location(&self.program, "instance_offset");
		let zindex_attribute_location = self.context.get_attrib_location(&self.program, "line_zindex");
		let width_attribute_location = self.context.get_attrib_location(&self.program, "line_width");

		assert_eq!(line_start_attribute_location, 0);
		assert_eq!(line_end_attribute_location, 1);
		assert_eq!(color_attribute_location, 2);
		assert_eq!(zindex_attribute_location, 3);
		assert_eq!(width_attribute_location, 4);
		assert_eq!(offset_matrix_attribute_location, 5);

		self.context.enable_vertex_attrib_array(line_start_attribute_location as u32);
		self.context.vertex_attrib_pointer_with_i32(0, 2, WebGl2RenderingContext::FLOAT, false, vertex_size, 0);
		self.context.vertex_attrib_divisor(line_start_attribute_location as u32, 1);
		self.context.enable_vertex_attrib_array(line_end_attribute_location as u32);
		self.context.vertex_attrib_pointer_with_i32(1, 2, WebGl2RenderingContext::FLOAT, false, vertex_size, float_size * 2);
		self.context.vertex_attrib_divisor(line_end_attribute_location as u32, 1);
		self.context.enable_vertex_attrib_array(color_attribute_location as u32);
		self.context.vertex_attrib_pointer_with_i32(2, 4, WebGl2RenderingContext::FLOAT, false, vertex_size, float_size * 4);
		self.context.vertex_attrib_divisor(color_attribute_location as u32, 1);
		self.context.enable_vertex_attrib_array(zindex_attribute_location as u32);
		self.context.vertex_attrib_pointer_with_i32(3, 1, WebGl2RenderingContext::FLOAT, false, vertex_size, float_size * 8);
		self.context.vertex_attrib_divisor(zindex_attribute_location as u32, 1);
		self.context.enable_vertex_attrib_array(width_attribute_location as u32);
		self.context.vertex_attrib_pointer_with_i32(4, 1, WebGl2RenderingContext::FLOAT, false, vertex_size, float_size * 9);
		self.context.vertex_attrib_divisor(width_attribute_location as u32, 1);
		for i in 0..3 {
			let location = offset_matrix_attribute_location as u32 + i;
			self.context.enable_vertex_attrib_array(location);
			self.context.vertex_attrib_divisor(location, 1);
			self.context
				.vertex_attrib_pointer_with_i32(location, 2, WebGl2RenderingContext::FLOAT, false, vertex_size, float_size * (10 + 2 * i as i32));
		}
		let vert_count = vertex_data.len() as i32;
		log::debug!("vert count {vert_count}");
		draw(&self.context, vert_count);

		Ok(())
	}
}

fn draw(context: &WebGl2RenderingContext, vert_count: i32) {
	context.clear_color(0.0, 0.0, 0.0, 0.0);
	//context.clear_color(1.0, 1.0, 1.0, 1.0);
	context.clear(WebGl2RenderingContext::COLOR_BUFFER_BIT);
	context.enable(WebGl2RenderingContext::DEPTH_TEST);
	context.depth_func(WebGl2RenderingContext::LESS);

	context.draw_arrays_instanced(WebGl2RenderingContext::TRIANGLE_STRIP, 0, 4, vert_count);
	//context.draw_arrays(WebGl2RenderingContext::TRIANGLES, 0, vert_count);
}

pub fn compile_shader(context: &WebGl2RenderingContext, shader_type: u32, source: &str) -> Result<WebGlShader, String> {
	let shader = context.create_shader(shader_type).ok_or_else(|| String::from("Unable to create shader object"))?;
	context.shader_source(&shader, source);
	context.compile_shader(&shader);

	if context.get_shader_parameter(&shader, WebGl2RenderingContext::COMPILE_STATUS).as_bool().unwrap_or(false) {
		Ok(shader)
	} else {
		Err(context.get_shader_info_log(&shader).unwrap_or_else(|| String::from("Unknown error creating shader")))
	}
}

pub fn link_program(context: &WebGl2RenderingContext, vert_shader: &WebGlShader, frag_shader: &WebGlShader) -> Result<WebGlProgram, String> {
	let program = context.create_program().ok_or_else(|| String::from("Unable to create shader object"))?;

	context.attach_shader(&program, vert_shader);
	context.attach_shader(&program, frag_shader);
	context.link_program(&program);

	if context.get_program_parameter(&program, WebGl2RenderingContext::LINK_STATUS).as_bool().unwrap_or(false) {
		Ok(program)
	} else {
		Err(context.get_program_info_log(&program).unwrap_or_else(|| String::from("Unknown error creating program object")))
	}
}
