use crate::color_palette::ColorPalette;
use crate::window_events;
use crate::pipeline::Pipeline;
use crate::texture::Texture;
use crate::resource_cache::ResourceCache;
use crate::gui_layout::GuiLayout;
use crate::gui_node::GuiNode;
use winit::event::*;
use winit::event_loop::*;
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
	pub gui_root: rctree::Node<GuiNode>,
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

		// Resource caches that own the application's shaders, pipelines, and textures
		let mut shader_cache = ResourceCache::<wgpu::ShaderModule>::new();
		let mut pipeline_cache = ResourceCache::<Pipeline>::new();
		let texture_cache = ResourceCache::<Texture>::new();

		// Temporary setup below, TODO: move to appropriate place in architecture

		// Data structure maintaining the user interface
		let gui_rect_pipeline = Pipeline::new(
			&device, swap_chain_descriptor.format, Vec::new(), &mut shader_cache, ("shaders/shader.vert", "shaders/shader.frag")
		);
		pipeline_cache.set("gui_rect", gui_rect_pipeline);

		// Render quad hierarchy
		let gui_root_data = GuiNode::new(swap_chain_descriptor.width, swap_chain_descriptor.height, ColorPalette::Accent.into_color_srgb());
		let gui_root = rctree::Node::new(gui_root_data);

		// Main window in the XML layout language
		let mut main_window_layout = GuiLayout::new();
		main_window_layout.load_layout("window", "main");

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
			gui_root,
		}
	}

	// Initializes the event loop for rendering and event handling
	pub fn begin_lifecycle(mut self, event_loop: EventLoop<()>, window: Window) {
		event_loop.run(move |event, _, control_flow| self.main_event_loop(event, control_flow, &window));
	}

	// Called every time by the event loop
	fn main_event_loop<T>(&mut self, event: Event<'_, T>, control_flow: &mut ControlFlow, window: &Window) {
		// Wait for the next event to cause a subsequent event loop run, instead of looping instantly as a game would need
		*control_flow = ControlFlow::Wait;

		match event {
			// Handle all window events (like input and resize) in sequence
			Event::WindowEvent { window_id, ref event } if window_id == window.id() => window_events::window_event(self, control_flow, event),
			// Handle raw hardware-related events not related to a window
			Event::DeviceEvent { .. } => (),
			// Handle custom-dispatched events
			Event::UserEvent(_) => (),
			// Called once every event is handled and the GUI structure is updated
			Event::MainEventsCleared => self.update_gui(),
			// Resizing or calling `window.request_redraw()` renders the GUI with the queued draw commands
			Event::RedrawRequested(_) => self.render(),
			// Once all windows have been redrawn
			Event::RedrawEventsCleared => (),
			Event::NewEvents(_) => (),
			Event::Suspended => (),
			Event::Resumed => (),
			Event::LoopDestroyed => (),
			_ => (),
		}
	}

	fn update_gui(&mut self) {

	}

	// Render the queue of pipeline draw commands over the current window
	fn render(&mut self) {
		// Get a frame buffer to render on
		let frame = self.swap_chain.get_next_texture().expect("Timeout getting frame buffer texture");
		
		// Generates a render pass that commands are applied to, then generates a command buffer when finished
		let mut command_encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Render Encoder") });

		// Build an array of draw commands by traversing the GUI element tree
		let commands = GuiNode::build_draw_commands_recursive(&self.gui_root, &self.device, &mut self.queue, &self.pipeline_cache, &mut self.texture_cache);

		// Recording of commands while in "rendering mode" that go into a command buffer
		let mut render_pass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
			color_attachments: &[
				wgpu::RenderPassColorAttachmentDescriptor {
					attachment: &frame.view,
					resolve_target: None,
					load_op: wgpu::LoadOp::Clear,
					store_op: wgpu::StoreOp::Store,
					clear_color: wgpu::Color::BLACK,
				}
			],
			depth_stencil_attachment: None,
		});

		// Prepare a variable to reuse the pipeline based on its name
		let mut pipeline_name = String::new();
		
		// Turn the queue of pipelines each into a command buffer and submit it to the render queue
		for i in 0..commands.len() {
			// If the previously set pipeline can't be reused, send the GPU the new pipeline to draw with
			if pipeline_name != commands[i].pipeline_name {
				let pipeline = self.pipeline_cache.get(&commands[i].pipeline_name[..]).unwrap();
				render_pass.set_pipeline(&pipeline.render_pipeline);
				pipeline_name = commands[i].pipeline_name.clone();
			}

			// Send the GPU the vertices and triangle indices
			render_pass.set_vertex_buffer(0, &commands[i].vertex_buffer, 0, 0);
			render_pass.set_index_buffer(&commands[i].index_buffer, 0, 0);

			// Send the GPU the bind group resources
			for (index, bind_group) in commands[i].bind_groups.iter().enumerate() {
				render_pass.set_bind_group(index as u32, bind_group, &[]);
			}

			// Draw call
			render_pass.draw_indexed(0..commands[i].index_count, 0, 0..1);
		};

		// Done sending render pass commands so we can give up mutation rights to command_encoder
		drop(render_pass);

		// Turn the recording of commands into a complete command buffer
		let command_buffer = command_encoder.finish();

		// Submit the command buffer to the GPU command queue
		self.queue.submit(&[command_buffer]);
	}
}
