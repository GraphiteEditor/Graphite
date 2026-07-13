use ipc_channel::ipc::IpcSender;
use std::ffi::CString;
use std::sync::{Arc, Mutex};

use mach2::kern_return::KERN_SUCCESS;
use mach2::message::{
	MACH_MSG_PORT_DESCRIPTOR, MACH_MSG_SUCCESS, MACH_MSG_TIMEOUT_NONE, MACH_MSG_TYPE_COPY_SEND, MACH_MSG_TYPE_MOVE_SEND, MACH_MSGH_BITS_COMPLEX, MACH_RCV_MSG, MACH_RCV_TIMED_OUT, MACH_RCV_TIMEOUT,
	MACH_SEND_MSG, mach_msg, mach_msg_body_t, mach_msg_header_t,
};
use mach2::port::{MACH_PORT_NULL, mach_port_t};
use mach2::traps::mach_task_self;
use objc2_io_surface::IOSurfaceRef;

use super::RecvResult;
use crate::frames::surface::FrameSurface;
use crate::remote::HostConfig;
use crate::remote::messages::EventMessage;

// From libSystem, stable since 10.0, not coverd by mach2
unsafe extern "C" {
	static bootstrap_port: mach_port_t;
	fn bootstrap_check_in(bp: mach_port_t, service_name: *const std::ffi::c_char, sp: *mut mach_port_t) -> mach2::kern_return::kern_return_t;
	fn bootstrap_look_up(bp: mach_port_t, service_name: *const std::ffi::c_char, sp: *mut mach_port_t) -> mach2::kern_return::kern_return_t;
}

// `mach_msg_port_descriptor_t` kernel ABI
#[repr(C)]
#[derive(Clone, Copy)]
struct PortDescriptor {
	name: mach_port_t,
	pad1: u32,
	pad2: u16,
	disposition: u8,
	descriptor_type: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FrameDescriptor {
	seq: u64,
	width: u32,
	height: u32,
	format: u32,
	content_x: u32,
	content_y: u32,
	content_width: u32,
	content_height: u32,
	source_width: u32,
	source_height: u32,
	_pad: u32,
}

#[repr(C)]
struct FrameMessage {
	header: mach_msg_header_t,
	body: mach_msg_body_t,
	surface: PortDescriptor,
	descriptor: FrameDescriptor,
}

#[repr(C)]
struct FrameMessageBuffer {
	message: FrameMessage,
	trailer: [u8; 64],
}

struct SendRight(mach_port_t);

// SAFETY: mach port names are task-wide; rights may be used from any thread.
unsafe impl Send for SendRight {}
unsafe impl Sync for SendRight {}

impl Drop for SendRight {
	fn drop(&mut self) {
		// SAFETY: we own one reference on this send right.
		unsafe { mach2::mach_port::mach_port_deallocate(mach_task_self(), self.0) };
	}
}

pub(crate) fn create_service(name: &str) -> std::io::Result<mach_port_t> {
	let c_name = CString::new(name).map_err(std::io::Error::other)?;
	let mut port: mach_port_t = MACH_PORT_NULL;
	// SAFETY: plain bootstrap call; on success we own the service's receive right.
	let result = unsafe { bootstrap_check_in(bootstrap_port, c_name.as_ptr(), &mut port) };
	if result != KERN_SUCCESS {
		return Err(std::io::Error::other(format!("bootstrap_check_in failed: {result:#x}")));
	}
	Ok(port)
}

fn look_up_service(name: &str) -> std::io::Result<SendRight> {
	let c_name = CString::new(name).map_err(std::io::Error::other)?;
	let mut port: mach_port_t = MACH_PORT_NULL;
	// SAFETY: plain bootstrap call; on success we own a send right.
	let result = unsafe { bootstrap_look_up(bootstrap_port, c_name.as_ptr(), &mut port) };
	if result != KERN_SUCCESS {
		return Err(std::io::Error::other(format!("bootstrap_look_up failed: {result:#x}")));
	}
	Ok(SendRight(port))
}

pub(crate) struct PlaneSender {
	service: SendRight,
}

impl PlaneSender {
	pub(crate) fn from_config(config: &HostConfig, _events: Arc<Mutex<IpcSender<EventMessage>>>) -> Option<Self> {
		let name = config.frame_service.as_deref()?;
		match look_up_service(name) {
			Ok(service) => Some(Self { service }),
			Err(e) => {
				tracing::error!("Failed to look up the accelerated frame service, falling back to software frames: {e}");
				None
			}
		}
	}

	pub(crate) fn stage(&self, info: &cef::AcceleratedPaintInfo) -> Option<StagedFrame> {
		let coded_size = &info.extra.coded_size;
		if coded_size.width <= 0 || coded_size.height <= 0 {
			tracing::error!("Accelerated paint delivered an invalid coded size: {}x{}", coded_size.width, coded_size.height);
			return None;
		}

		let Some(surface) = std::ptr::NonNull::new(info.shared_texture_io_surface.cast::<IOSurfaceRef>()) else {
			tracing::error!("Accelerated paint delivered a null IOSurface");
			return None;
		};

		// SAFETY: CEF keeps the surface valid for the `on_accelerated_paint` callback.
		let port = unsafe { surface.as_ref() }.create_mach_port();
		if port == MACH_PORT_NULL {
			tracing::error!("Failed to wrap the IOSurface in a mach port");
			return None;
		}

		let content = crate::frames::import::ContentRect::try_from(info).unwrap_or_default();
		Some(StagedFrame {
			descriptor: FrameDescriptor {
				seq: 0,
				width: coded_size.width as u32,
				height: coded_size.height as u32,
				format: *info.format.as_ref() as u32,
				content_x: content.x,
				content_y: content.y,
				content_width: content.width,
				content_height: content.height,
				source_width: content.source_width,
				source_height: content.source_height,
				_pad: 0,
			},
			surface: SendRight(port),
		})
	}

	pub(crate) fn send(&self, seq: u64, frame: StagedFrame) -> std::io::Result<()> {
		let mut descriptor = frame.descriptor;
		descriptor.seq = seq;
		let mut message = FrameMessage {
			header: mach_msg_header_t {
				msgh_bits: MACH_MSG_TYPE_COPY_SEND | MACH_MSGH_BITS_COMPLEX,
				msgh_size: std::mem::size_of::<FrameMessage>() as u32,
				msgh_remote_port: self.service.0,
				msgh_local_port: MACH_PORT_NULL,
				msgh_voucher_port: MACH_PORT_NULL,
				msgh_id: 0,
			},
			body: mach_msg_body_t { msgh_descriptor_count: 1 },
			surface: PortDescriptor {
				name: frame.surface.0,
				pad1: 0,
				pad2: 0,
				disposition: MACH_MSG_TYPE_MOVE_SEND as u8,
				descriptor_type: MACH_MSG_PORT_DESCRIPTOR as u8,
			},
			descriptor,
		};

		// SAFETY: message is a well-formed complex message of the declared size.
		let result = unsafe {
			mach_msg(
				&mut message.header,
				MACH_SEND_MSG,
				std::mem::size_of::<FrameMessage>() as u32,
				0,
				MACH_PORT_NULL,
				MACH_MSG_TIMEOUT_NONE,
				MACH_PORT_NULL,
			)
		};
		if result != MACH_MSG_SUCCESS {
			return Err(std::io::Error::other(format!("mach_msg send failed: {result:#x}")));
		}

		// Kernel took ownership of the surface and moves it to the receiver. We must not drop.
		std::mem::forget(frame.surface);

		Ok(())
	}
}

pub(crate) struct StagedFrame {
	descriptor: FrameDescriptor,
	surface: SendRight,
}

pub(crate) struct PlaneReceiver {
	port: mach_port_t,
}

impl PlaneReceiver {
	pub(crate) fn new(port: mach_port_t) -> Self {
		Self { port }
	}

	pub(crate) fn recv_blocking(&self) -> std::io::Result<RecvResult> {
		self.recv(false)
	}

	pub(crate) fn try_recv(&self) -> std::io::Result<RecvResult> {
		self.recv(true)
	}

	fn recv(&self, nonblocking: bool) -> std::io::Result<RecvResult> {
		// SAFETY: zeroed is a valid representation for these plain-data structs.
		let mut buffer: FrameMessageBuffer = unsafe { std::mem::zeroed() };
		let (options, timeout) = if nonblocking {
			(MACH_RCV_MSG | MACH_RCV_TIMEOUT, 0)
		} else {
			(MACH_RCV_MSG, MACH_MSG_TIMEOUT_NONE)
		};

		// SAFETY: the buffer is large enough for the message plus the basic trailer.
		let result = unsafe {
			mach_msg(
				&mut buffer.message.header,
				options,
				0,
				std::mem::size_of::<FrameMessageBuffer>() as u32,
				self.port,
				timeout,
				MACH_PORT_NULL,
			)
		};
		if result == MACH_RCV_TIMED_OUT {
			return Ok(RecvResult::WouldBlock);
		}
		if result != MACH_MSG_SUCCESS {
			return Err(std::io::Error::other(format!("mach_msg receive failed: {result:#x}")));
		}

		let received_complex = buffer.message.header.msgh_bits & MACH_MSGH_BITS_COMPLEX != 0;
		let descriptor_count = if received_complex { buffer.message.body.msgh_descriptor_count } else { 0 };
		let surface = (descriptor_count == 1 && buffer.message.surface.descriptor_type == MACH_MSG_PORT_DESCRIPTOR as u8).then(|| SendRight(buffer.message.surface.name));

		if buffer.message.header.msgh_size as usize != std::mem::size_of::<FrameMessage>() {
			return Err(std::io::Error::other(format!("malformed frame message: {} bytes", buffer.message.header.msgh_size)));
		}
		let Some(surface) = surface else {
			return Err(std::io::Error::other("frame message carried no surface port"));
		};

		Ok(RecvResult::Frame(WireFrame {
			descriptor: buffer.message.descriptor,
			surface,
		}))
	}
}

pub(crate) struct WireFrame {
	descriptor: FrameDescriptor,
	surface: SendRight,
}

impl WireFrame {
	pub(crate) fn seq(&self) -> u64 {
		self.descriptor.seq
	}

	pub(crate) fn import(self, surface: &FrameSurface) -> Option<wgpu::Texture> {
		let WireFrame { descriptor, surface: port } = self;
		let format = super::wire_color_type(descriptor.format)?;

		// Lookup takes its own reference on the surface, port can be dropped.
		let io_surface = IOSurfaceRef::lookup_from_mach_port(port.0);
		drop(port);

		let Some(io_surface) = io_surface else {
			tracing::error!("Failed to look up the IOSurface for frame {}", descriptor.seq);
			return None;
		};
		let io_surface_ref: &IOSurfaceRef = &io_surface;

		let content = crate::frames::import::ContentRect {
			x: descriptor.content_x,
			y: descriptor.content_y,
			width: descriptor.content_width,
			height: descriptor.content_height,
			source_width: descriptor.source_width,
			source_height: descriptor.source_height,
		};
		let importer = crate::frames::import::iosurface::IOSurfaceImporter::from_parts(io_surface_ref as *const _ as *mut std::os::raw::c_void, descriptor.width, descriptor.height, format);
		surface.import_texture(importer, content)
	}
}
