use ipc_channel::ipc::IpcSender;
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};
use std::sync::{Arc, Mutex};

use super::RecvResult;
use crate::frames::surface::FrameSurface;
use crate::remote::HostConfig;
use crate::remote::messages::EventMessage;
pub(crate) const FRAME_SOCKET_CHILD_FD: RawFd = 3;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct FrameDescriptor {
	seq: u64,
	modifier: u64,
	width: u32,
	height: u32,
	format: u32,
	plane_count: u32,
	strides: [u32; 4],
	offsets: [u32; 4],
}

const DESCRIPTOR_BYTES: usize = std::mem::size_of::<FrameDescriptor>();
const MAX_PLANES: usize = 4;

pub(crate) fn socketpair() -> std::io::Result<(OwnedFd, OwnedFd)> {
	let mut fds = [0 as RawFd; 2];
	// SAFETY: socketpair call; on success fds are owned by us.
	let result = unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC, 0, fds.as_mut_ptr()) };
	if result != 0 {
		return Err(std::io::Error::last_os_error());
	}
	// SAFETY: socketpair succeeded, so both fds are valid and not owned elsewhere.
	Ok(unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) })
}

pub(crate) struct PlaneSender {
	socket: OwnedFd,
}

impl PlaneSender {
	pub(crate) fn from_config(config: &HostConfig, _events: Arc<Mutex<IpcSender<EventMessage>>>) -> Option<Self> {
		let fd = config.frame_socket_fd?;
		// SAFETY: the spawner dup2'd this fd for us; nothing else owns it.
		let socket = unsafe { OwnedFd::from_raw_fd(fd) };

		// Restore CLOEXEC so subprocesses don't inherit the socket.
		// SAFETY: plain fcntl on an fd we own.
		if unsafe { libc::fcntl(socket.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } != 0 {
			tracing::warn!("Failed to set CLOEXEC on the frame socket: {}", std::io::Error::last_os_error());
		}
		Some(Self { socket })
	}

	pub(crate) fn stage(&self, info: &cef::AcceleratedPaintInfo) -> Option<StagedFrame> {
		let plane_count = (info.plane_count.max(0) as usize).min(info.planes.len());
		let mut fds = Vec::with_capacity(plane_count);
		let mut strides = [0u32; 4];
		let mut offsets = [0u32; 4];
		for (i, plane) in info.planes[..plane_count].iter().enumerate() {
			// SAFETY: CEF keeps the plane fds valid for the `on_accelerated_paint` callback.
			let fd = unsafe { BorrowedFd::borrow_raw(plane.fd) };
			match fd.try_clone_to_owned() {
				Ok(owned) => fds.push(owned),
				Err(e) => {
					tracing::error!("Failed to duplicate DMA-BUF plane fd: {e}");
					return None;
				}
			}
			strides[i] = plane.stride;
			offsets[i] = plane.offset as u32;
		}

		Some(StagedFrame {
			descriptor: FrameDescriptor {
				seq: 0,
				modifier: info.modifier,
				width: info.extra.coded_size.width as u32,
				height: info.extra.coded_size.height as u32,
				format: *info.format.as_ref() as u32,
				plane_count: plane_count as u32,
				strides,
				offsets,
			},
			fds,
		})
	}

	pub(crate) fn send(&self, seq: u64, frame: StagedFrame) -> std::io::Result<()> {
		let mut descriptor = frame.descriptor;
		descriptor.seq = seq;

		debug_assert!(frame.fds.len() <= MAX_PLANES);
		let fd_bytes = frame.fds.len() * std::mem::size_of::<RawFd>();
		let mut iov = libc::iovec {
			iov_base: &descriptor as *const FrameDescriptor as *mut libc::c_void,
			iov_len: DESCRIPTOR_BYTES,
		};
		let mut cmsg_buffer = [0u8; unsafe { libc::CMSG_SPACE((MAX_PLANES * std::mem::size_of::<RawFd>()) as u32) } as usize];
		let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
		msg.msg_iov = &mut iov;
		msg.msg_iovlen = 1;
		msg.msg_control = cmsg_buffer.as_mut_ptr().cast();
		msg.msg_controllen = unsafe { libc::CMSG_SPACE(fd_bytes as u32) } as _;
		unsafe {
			let cmsg = libc::CMSG_FIRSTHDR(&msg);
			(*cmsg).cmsg_level = libc::SOL_SOCKET;
			(*cmsg).cmsg_type = libc::SCM_RIGHTS;
			(*cmsg).cmsg_len = libc::CMSG_LEN(fd_bytes as u32) as _;
			let data = libc::CMSG_DATA(cmsg) as *mut RawFd;
			for (i, fd) in frame.fds.iter().enumerate() {
				data.add(i).write_unaligned(fd.as_raw_fd());
			}
		}
		loop {
			// SAFETY: msg and everything it points to are valid for the duration of the call.
			let sent = unsafe { libc::sendmsg(self.socket.as_raw_fd(), &msg, libc::MSG_NOSIGNAL) };
			if sent >= 0 {
				return Ok(());
			}
			let error = std::io::Error::last_os_error();
			if error.kind() != std::io::ErrorKind::Interrupted {
				return Err(error);
			}
		}
	}
}

pub(crate) struct StagedFrame {
	descriptor: FrameDescriptor,
	fds: Vec<OwnedFd>,
}

pub(crate) struct PlaneReceiver {
	socket: OwnedFd,
}

impl PlaneReceiver {
	pub(crate) fn new(socket: OwnedFd) -> Self {
		Self { socket }
	}

	pub(crate) fn recv_blocking(&self) -> std::io::Result<RecvResult> {
		self.recv(false)
	}

	pub(crate) fn try_recv(&self) -> std::io::Result<RecvResult> {
		self.recv(true)
	}

	fn recv(&self, nonblocking: bool) -> std::io::Result<RecvResult> {
		let mut descriptor: FrameDescriptor = bytemuck::Zeroable::zeroed();
		let mut iov = libc::iovec {
			iov_base: (&mut descriptor as *mut FrameDescriptor).cast(),
			iov_len: DESCRIPTOR_BYTES,
		};
		// SAFETY: pure size computation.
		let mut cmsg_buffer = [0u8; unsafe { libc::CMSG_SPACE((MAX_PLANES * std::mem::size_of::<RawFd>()) as u32) } as usize];

		let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
		msg.msg_iov = &mut iov;
		msg.msg_iovlen = 1;
		msg.msg_control = cmsg_buffer.as_mut_ptr().cast();
		msg.msg_controllen = cmsg_buffer.len() as _;

		let flags = libc::MSG_CMSG_CLOEXEC | if nonblocking { libc::MSG_DONTWAIT } else { 0 };
		let received = loop {
			// SAFETY: msg and everything it points to are valid for the duration of the call.
			let received = unsafe { libc::recvmsg(self.socket.as_raw_fd(), &mut msg, flags) };
			if received >= 0 {
				break received;
			}
			let error = std::io::Error::last_os_error();
			match error.kind() {
				std::io::ErrorKind::Interrupted => continue,
				std::io::ErrorKind::WouldBlock => return Ok(RecvResult::WouldBlock),
				_ => return Err(error),
			}
		};

		let mut fds = Vec::new();
		// SAFETY: traversing the cmsgs recvmsg just filled; SCM_RIGHTS payload is fds now owned by us.
		unsafe {
			let mut cmsg = libc::CMSG_FIRSTHDR(&msg);
			while !cmsg.is_null() {
				if (*cmsg).cmsg_level == libc::SOL_SOCKET && (*cmsg).cmsg_type == libc::SCM_RIGHTS {
					let data = libc::CMSG_DATA(cmsg) as *const RawFd;
					let count = ((*cmsg).cmsg_len as usize - libc::CMSG_LEN(0) as usize) / std::mem::size_of::<RawFd>();
					for i in 0..count {
						fds.push(OwnedFd::from_raw_fd(data.add(i).read_unaligned()));
					}
				}
				cmsg = libc::CMSG_NXTHDR(&msg, cmsg);
			}
		}

		if received == 0 {
			return Ok(RecvResult::Closed);
		}
		if received as usize != DESCRIPTOR_BYTES || (msg.msg_flags & libc::MSG_CTRUNC) != 0 {
			return Err(std::io::Error::other(format!(
				"malformed frame message: {received} bytes, flags {:#x} ({} fds)",
				msg.msg_flags,
				fds.len()
			)));
		}

		Ok(RecvResult::Frame(WireFrame { descriptor, fds }))
	}
}

pub(crate) struct WireFrame {
	descriptor: FrameDescriptor,
	fds: Vec<OwnedFd>,
}

impl WireFrame {
	pub(crate) fn seq(&self) -> u64 {
		self.descriptor.seq
	}

	pub(crate) fn import(self, surface: &FrameSurface) -> Option<wgpu::Texture> {
		let descriptor = self.descriptor;
		let format = super::wire_color_type(descriptor.format)?;
		let plane_count = (descriptor.plane_count as usize).min(self.fds.len());
		let importer = crate::frames::import::dmabuf::DmaBufImporter::from_parts(
			self.fds,
			descriptor.strides[..plane_count].to_vec(),
			descriptor.offsets[..plane_count].to_vec(),
			descriptor.modifier,
			descriptor.width,
			descriptor.height,
			format,
		);
		surface.import_texture(importer)
	}
}
