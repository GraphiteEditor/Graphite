#[cfg(feature = "accelerated_paint")]
pub fn should_enable_hardware_acceleration() -> bool {
	#[cfg(target_os = "linux")]
	{
		// Check if running on Wayland or X11
		let has_wayland = std::env::var("WAYLAND_DISPLAY")
			.ok()
			.filter(|var| !var.is_empty())
			.or_else(|| std::env::var("WAYLAND_SOCKET").ok())
			.filter(|var| !var.is_empty())
			.is_some();

		let has_x11 = std::env::var("DISPLAY").ok().filter(|var| !var.is_empty()).is_some();

		if !has_wayland && !has_x11 {
			tracing::warn!("No display server detected, disabling hardware acceleration");
			return false;
		}

		// Check for NVIDIA proprietary driver (known to have issues)
		if let Ok(driver_info) = std::fs::read_to_string("/proc/driver/nvidia/version") {
			if driver_info.contains("NVIDIA") {
				tracing::warn!("NVIDIA proprietary driver detected, hardware acceleration may be unstable");
				// Still return true but with warning
			}
		}

		// Check for basic GPU capabilities
		if has_wayland {
			tracing::info!("Wayland detected, enabling hardware acceleration");
			true
		} else if has_x11 {
			tracing::info!("X11 detected, enabling hardware acceleration");
			true
		} else {
			false
		}
	}

	#[cfg(target_os = "windows")]
	{
		// Windows generally has good D3D11 support
		tracing::info!("Windows detected, enabling hardware acceleration");
		true
	}

	#[cfg(target_os = "macos")]
	{
		// macOS has good Metal/IOSurface support
		tracing::info!("macOS detected, enabling hardware acceleration");
		true
	}

	#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
	{
		tracing::warn!("Unsupported platform for hardware acceleration");
		false
	}
}
