// use super::render_state::RenderState;
// use super::program_state::ProgramState;
use super::color_palette::ColorPalette;
use super::gui_rect::GUIRect;
use super::pipeline::Pipeline;
use super::pipeline::PipelineDetails;
use super::shader_cache::ShaderCache;
use super::texture::Texture;

use std::collections::VecDeque;
use winit::event::*;
use winit::event_loop::ControlFlow;
use winit::event_loop::EventLoop;
use winit::window::Window;

pub struct Application {
	pub surface: wgpu::Surface,
	pub adapter: wgpu::Adapter,
	pub device: wgpu::Device,
	pub queue: wgpu::Queue,
	pub swap_chain_descriptor: wgpu::SwapChainDescriptor,
	pub swap_chain: wgpu::SwapChain,
	pub shader_cache: ShaderCache,
	// pub texture_cache: TextureCache,
	pub gui_rect_queue: VecDeque<GUIRect>,
	pub pipeline_queue: VecDeque<Pipeline>,
	pub temp_color_toggle: bool,
}

impl Application {
	pub fn new(window: &Window) -> Self {
		// Window as understood by WGPU for rendering onto
		let surface = wgpu::Surface::create(window);

		// Represents a GPU, exposes the real GPU device and queue
		let adapter = wgpu::Adapter::request(&wgpu::RequestAdapterOptions { ..Default::default() }).unwrap();

		// Requests the device and queue from the adapter
		let requested_device = adapter.request_device(&wgpu::DeviceDescriptor {
			extensions: wgpu::Extensions { anisotropic_filtering: false },
			limits: Default::default(),
		});

		// Connection to the physical GPU
		let device = requested_device.0;

		// Represents the GPU command queue, to submit CommandBuffers
		let queue = requested_device.1;
		
		// Properties for the swap chain frame buffers
		let swap_chain_descriptor = wgpu::SwapChainDescriptor {
			usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
			format: wgpu::TextureFormat::Bgra8UnormSrgb,
			width: window.inner_size().width,
			height: window.inner_size().height,
			present_mode: wgpu::PresentMode::Vsync,
		};

		// Series of frame buffers with images presented to the surface
		let swap_chain = device.create_swap_chain(&surface, &swap_chain_descriptor);

		// Cache of all loaded shaders
		let shader_cache = ShaderCache::new();

		let gui_rect_queue = VecDeque::new();

		let pipeline_queue = VecDeque::new();
		
		Self {
			surface,
			adapter,
			device,
			queue,
			swap_chain_descriptor,
			swap_chain,
			shader_cache,
			gui_rect_queue,
			pipeline_queue,
			temp_color_toggle: true,
		}
	}

	pub fn example(&mut self) {
		self.shader_cache.load(&self.device, "shaders/shader.vert", glsl_to_spirv::ShaderType::Vertex).unwrap();
		self.shader_cache.load(&self.device, "shaders/shader.frag", glsl_to_spirv::ShaderType::Fragment).unwrap();

		let vertex_shader = self.shader_cache.get_by_path("shaders/shader.vert").unwrap();
		let fragment_shader = self.shader_cache.get_by_path("shaders/shader.frag").unwrap();

		let texture_view = Texture::from_filepath(&self.device, &mut self.queue, "textures/grid.png").unwrap().view;

		let example_pipeline = Pipeline::new(&self.device, PipelineDetails {
			vertex_shader,
			fragment_shader,
			texture_view: Some(&texture_view),
		});

		self.pipeline_queue.push_back(example_pipeline);
	}

	pub fn begin_lifecycle(mut self, event_loop: EventLoop<()>, window: Window) {
		event_loop.run(move |event, _, control_flow| self.main_event_loop(event, control_flow, &window));
	}

	pub fn main_event_loop<T>(&mut self, event: Event<'_, T>, control_flow: &mut ControlFlow, window: &Window) {
		match event {
			// Handle all window events in sequence
			Event::WindowEvent { ref event, window_id } if window_id == window.id() => {
				self.window_event(event, control_flow);
			},
			// After handling every event and updating the GUI, request a new sequence of draw commands
			Event::MainEventsCleared => {
				// Turn the GUI changes into draw commands added to the render pipeline queue
				self.redraw();

				// If any draw commands were actually added, ask the window to issue a redraw event
				if !self.pipeline_queue.is_empty() {
					window.request_redraw();
				}

				*control_flow = ControlFlow::Wait;
			},
			// Resizing or calling `window.request_redraw()` now redraws the GUI with the pipeline queue
			Event::RedrawRequested(_) => {
				self.render();
				*control_flow = ControlFlow::Wait;
			},
			// Catch extraneous events
			_ => {
				*control_flow = ControlFlow::Wait;
			},
		}
	}

	pub fn window_event(&mut self, event: &WindowEvent, control_flow: &mut ControlFlow) {
		match event {
			WindowEvent::CloseRequested => {
				self.quit(control_flow);
			},
			WindowEvent::KeyboardInput { input, .. } => {
				self.keyboard_event(input, control_flow);
			},
			WindowEvent::Resized(physical_size) => {
				self.resize(*physical_size);
				*control_flow = ControlFlow::Wait;
			},
			WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
				self.resize(**new_inner_size);
				*control_flow = ControlFlow::Wait;
			},
			_ => {
				*control_flow = ControlFlow::Wait;
			},
		}
	}

	pub fn keyboard_event(&mut self, input: &KeyboardInput, control_flow: &mut ControlFlow) {
		match input {
			KeyboardInput { state: ElementState::Pressed, virtual_keycode: Some(VirtualKeyCode::Escape), .. } => {
				self.quit(control_flow);
			},
			KeyboardInput { state: ElementState::Pressed, virtual_keycode: Some(VirtualKeyCode::Space), .. } => {
				self.example();
			},
			_ => {
				*control_flow = ControlFlow::Wait;
			},
		}
	}

	pub fn quit(&self, control_flow: &mut ControlFlow) {
		*control_flow = ControlFlow::Exit;
	}

	pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
		self.swap_chain_descriptor.width = new_size.width;
		self.swap_chain_descriptor.height = new_size.height;

		self.swap_chain = self.device.create_swap_chain(&self.surface, &self.swap_chain_descriptor);

		// TODO: Mark root of GUI as dirty to force redraw of everything
	}

	// Traverse the dirty GUI elements and queue up pipelines to render each GUI rectangle (box/sprite)
	pub fn redraw(&mut self) {

	}

	// Render the queue of pipeline draw commands over the current window
	pub fn render(&mut self) {
		// Turn the queue of pipelines each into a command buffer and submit it to the render queue
		while !self.pipeline_queue.is_empty() {
			// Get a frame buffer to render on
			let frame = self.swap_chain.get_next_texture();
			
			// Get the pipeline to render in this iteration
			let pipeline_struct = self.pipeline_queue.pop_back().unwrap();

			// Generates a render pass that commands are applied to, then generates a command buffer when finished
			let mut command_encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { todo: 0 });

			// Temporary way to swap clear color every render
			let color = match self.temp_color_toggle {
				true => ColorPalette::get_color_linear(ColorPalette::MildBlack),
				false => ColorPalette::get_color_linear(ColorPalette::NearBlack),
			};
			self.temp_color_toggle = !self.temp_color_toggle;

			// Recording of commands while in "rendering mode" that go into a command buffer
			let mut render_pass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				color_attachments: &[
					wgpu::RenderPassColorAttachmentDescriptor {
						attachment: &frame.view,
						resolve_target: None,
						load_op: wgpu::LoadOp::Clear,
						store_op: wgpu::StoreOp::Store,
						clear_color: color,
					}
				],
				depth_stencil_attachment: None,
			});

			// Commands sent to the GPU for drawing during this render pass
			render_pass.set_pipeline(&pipeline_struct.render_pipeline);
			render_pass.set_vertex_buffers(0, &[(&pipeline_struct.vertex_buffer, 0)]);
			render_pass.set_index_buffer(&pipeline_struct.index_buffer, 0);
			render_pass.set_bind_group(0, &pipeline_struct.texture_bind_group, &[]);
			render_pass.draw_indexed(0..pipeline_struct.index_count, 0, 0..1);

			// Done sending render pass commands so we can give up mutation rights to command_encoder
			drop(render_pass);

			// Turn the recording of commands into a complete command buffer
			let command_buffer = command_encoder.finish();
			
			// Submit the command buffer to the GPU command queue
			self.queue.submit(&[command_buffer]);
		}
	}
}
