// use super::render_state::RenderState;
use super::color_palette::ColorPalette;
use super::gui_rect::GUIRect;
use super::pipeline::Pipeline;
use super::texture::Texture;
use super::shader_stage::compile_from_glsl;
use super::resource_cache::ResourceCache;
use super::draw_command::DrawCommand;
use std::collections::VecDeque;
use winit::event::*;
use winit::event_loop::ControlFlow;
use winit::event_loop::EventLoop;
use winit::window::Window;
use futures::executor::block_on;

pub struct Application {
	pub surface: wgpu::Surface,
	pub adapter: wgpu::Adapter,
	pub device: wgpu::Device,
	pub queue: wgpu::Queue,
	pub swap_chain_descriptor: wgpu::SwapChainDescriptor,
	pub swap_chain: wgpu::SwapChain,
	pub shader_cache: ResourceCache<wgpu::ShaderModule>,
	pub pipeline_cache: ResourceCache<Pipeline>,
	pub texture_cache: ResourceCache<Texture>,
	pub gui_rect_queue: VecDeque<GUIRect>,
	pub draw_command_queue: VecDeque<DrawCommand>,
	pub temp_color_toggle: bool,
}

impl Application {
	pub fn new(window: &Window) -> Self {
		// Window as understood by WGPU for rendering onto
		let surface = wgpu::Surface::create(window);

		// Represents a GPU, exposes the real GPU device and queue
		let adapter = block_on(wgpu::Adapter::request(
			&wgpu::RequestAdapterOptions {
				power_preference: wgpu::PowerPreference::Default,
				compatible_surface: Some(&surface),
			},
			wgpu::BackendBit::PRIMARY,
		)).unwrap();

		// Requests the device and queue from the adapter
		let requested_device = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
			extensions: wgpu::Extensions { anisotropic_filtering: false },
			limits: Default::default(),
		}));

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
			present_mode: wgpu::PresentMode::Fifo,
		};

		// Series of frame buffers with images presented to the surface
		let swap_chain = device.create_swap_chain(&surface, &swap_chain_descriptor);

		// Cache of all loaded shaders and the Pipeline programs they form
		let shader_cache = ResourceCache::<wgpu::ShaderModule>::new();
		let pipeline_cache = ResourceCache::<Pipeline>::new();
		let texture_cache = ResourceCache::<Texture>::new();

		let gui_rect_queue = VecDeque::new();

		let draw_command_queue = VecDeque::new();
		
		Self {
			surface,
			adapter,
			device,
			queue,
			swap_chain_descriptor,
			swap_chain,
			shader_cache,
			pipeline_cache,
			texture_cache,
			gui_rect_queue,
			draw_command_queue,
			temp_color_toggle: true,
		}
	}

	pub fn example(&mut self) {
		// Example vertex data
		const VERTICES: &[[f32; 2]] = &[
			[-0.0868241, 0.49240386],
			[-0.49513406, 0.06958647],
			[-0.21918549, -0.44939706],
			[0.35966998, -0.3473291],
			[0.44147372, 0.2347359],
		];
		const INDICES: &[u16] = &[
			0, 1, 4,
			1, 2, 4,
			2, 3, 4,
		];

		// Load the vertex shader
		let vertex_shader_path = "shaders/shader.vert";
		let vertex_shader_module = compile_from_glsl(&self.device, vertex_shader_path, glsl_to_spirv::ShaderType::Vertex).unwrap();
		self.shader_cache.set(vertex_shader_path, vertex_shader_module);

		// Load the fragment shader
		let fragment_shader_path = "shaders/shader.frag";
		let fragment_shader_module = compile_from_glsl(&self.device, fragment_shader_path, glsl_to_spirv::ShaderType::Fragment).unwrap();
		self.shader_cache.set(fragment_shader_path, fragment_shader_module);

		// Get the shader pair
		let vertex_shader = self.shader_cache.get(vertex_shader_path).unwrap();
		let fragment_shader = self.shader_cache.get(fragment_shader_path).unwrap();

		// Construct a pipeline from the shader pair
		let pipeline_name = "example";
		let pipeline = Pipeline::new(&self.device, vertex_shader, fragment_shader);
		self.pipeline_cache.set(pipeline_name, pipeline);
		let example_pipeline = self.pipeline_cache.get(pipeline_name).unwrap();
		
		// Load a texture from the image file
		let texture_path = "textures/grid.png";
		let texture = Texture::from_filepath(&self.device, &mut self.queue, texture_path).unwrap();
		self.texture_cache.set(texture_path, texture);
		let grid_texture = self.texture_cache.get(texture_path).unwrap();
		
		// Create a BindGroup that holds a new TextureView
		let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
			layout: &example_pipeline.bind_group_layout,
			bindings: &[
				wgpu::Binding {
					binding: 0,
					resource: wgpu::BindingResource::TextureView(&grid_texture.texture_view),
				},
				// wgpu::Binding {
				// 	binding: 1,
				// 	resource: wgpu::BindingResource::Sampler(&texture.sampler),
				// }
			],
			label: None,
		});

		// Create a draw command with the vertex data and bind group and push it to the GPU command queue
		let draw_command = DrawCommand::new(&self.device, pipeline_name, VERTICES, INDICES, bind_group);
		self.draw_command_queue.push_back(draw_command);
	}

	pub fn begin_lifecycle(mut self, event_loop: EventLoop<()>, window: Window) {
		event_loop.run(move |event, _, control_flow| self.main_event_loop(event, control_flow, &window));
	}

	pub fn main_event_loop<T>(&mut self, event: Event<'_, T>, control_flow: &mut ControlFlow, window: &Window) {
		// Wait for the next event to cause a subsequent event loop run, instead of looping instantly as a game would need
		*control_flow = ControlFlow::Wait;

		match event {
			// Handle all window events (like input and resize) in sequence
			Event::WindowEvent { ref event, window_id } if window_id == window.id() => {
				self.window_event(event, control_flow);
			},
			// Once every event is handled and the GUI structure is updated, this requests a new sequence of draw commands
			Event::MainEventsCleared => {
				// Turn the GUI changes into draw commands added to the render pipeline queue
				self.redraw_gui();

				// If any draw commands were actually added, ask the window to dispatch a redraw event
				if !self.draw_command_queue.is_empty() {
					window.request_redraw();
				}
			},
			// Resizing or calling `window.request_redraw()` renders the GUI with the queued draw commands
			Event::RedrawRequested(_) => {
				self.render();
			},
			// Catch extraneous events
			_ => {},
		}
	}

	pub fn window_event(&mut self, event: &WindowEvent, control_flow: &mut ControlFlow) {
		match event {
			WindowEvent::CloseRequested => self.quit(control_flow),
			WindowEvent::KeyboardInput { input, .. } => self.keyboard_event(input, control_flow),
			WindowEvent::Resized(physical_size) => self.resize(*physical_size),
			WindowEvent::ScaleFactorChanged { new_inner_size, .. } => self.resize(**new_inner_size),
			_ => {},
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
	pub fn redraw_gui(&mut self) {

	}

	// Render the queue of pipeline draw commands over the current window
	pub fn render(&mut self) {
		// Get a frame buffer to render on
		let frame = self.swap_chain.get_next_texture().expect("Timeout getting frame buffer texture");
		
		// Generates a render pass that commands are applied to, then generates a command buffer when finished
		let mut command_encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Render Encoder") });

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

		// let mut currently_set_pipeline_id = None;

		println!("Draw queue is length {}", self.draw_command_queue.len());

		// Turn the queue of pipelines each into a command buffer and submit it to the render queue
		self.draw_command_queue.iter().for_each(|command| {
			// // Bind the pipeline required by the current draw command
			// let new_pipeline_id = command.pipeline_id;
			// if currently_set_pipeline_id == None || new_pipeline_id != currently_set_pipeline_id.unwrap() {
			// 	currently_set_pipeline_id = Some(new_pipeline_id);

			// 	let pipeline = self.pipeline_cache.get_by_id(new_pipeline_id).unwrap();
			// 	render_pass.set_pipeline(&pipeline.render_pipeline);
			// 	println!("Set pipeline");
			// }

			let pipeline = self.pipeline_cache.get(&command.pipeline_name).unwrap();
			render_pass.set_pipeline(&pipeline.render_pipeline);
			
			// Commands sent to the GPU for drawing during this render pass
			render_pass.set_vertex_buffer(0, &command.vertex_buffer, 0, 0);
			render_pass.set_index_buffer(&command.index_buffer, 0, 0);
			render_pass.set_bind_group(0, &command.bind_group, &[]);

			// Draw call
			render_pass.draw_indexed(0..command.index_count, 0, 0..1);
			println!("Draw call!");
		});

		// Done sending render pass commands so we can give up mutation rights to command_encoder
		drop(render_pass);

		// Turn the recording of commands into a complete command buffer
		let command_buffer = command_encoder.finish();

		// After the draw command queue has been iterated through and used, empty it for use next frame
		self.draw_command_queue.clear();
		
		// Submit the command buffer to the GPU command queue
		self.queue.submit(&[command_buffer]);
	}
}
