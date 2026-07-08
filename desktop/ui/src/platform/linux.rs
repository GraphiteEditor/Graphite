use std::os::unix::process::CommandExt;
use std::process::Command;

#[cfg(feature = "accelerated_paint")]
use crate::frames::plane;

pub(crate) fn setup_command(command: &mut Command, #[cfg(feature = "accelerated_paint")] host_frame_fd: Option<std::os::fd::RawFd>) {
	let parent_pid = std::process::id() as libc::pid_t;
	// SAFETY: the closure runs in the forked child before exec and only makes async-signal-safe calls
	unsafe {
		command.pre_exec(move || {
			// Tie the host's lifetime to the parent process
			if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 {
				return Err(std::io::Error::last_os_error());
			}
			if libc::getppid() != parent_pid {
				return Err(std::io::Error::other("main process died before PDEATHSIG was set"));
			}

			// Move the host end of the frame socket to its advertised fd
			#[cfg(feature = "accelerated_paint")]
			if let Some(fd) = host_frame_fd {
				let target = plane::FRAME_SOCKET_CHILD_FD;
				if fd == target {
					if libc::fcntl(target, libc::F_SETFD, 0) != 0 {
						return Err(std::io::Error::last_os_error());
					}
				} else if libc::dup2(fd, target) == -1 {
					return Err(std::io::Error::last_os_error());
				}
			}
			Ok(())
		});
	}
}
