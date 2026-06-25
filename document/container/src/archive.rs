//! Archive codecs (zip, xz).
//!
//! Each codec streams entries in both directions: writers wrap an `io::Write` sink, and
//! `deserialize` reads from any `io::Read + Seek` source and streams entries into any [`Container`].

#[cfg(any(feature = "xz", feature = "zip"))]
use crate::AsyncContainer;
#[cfg(any(feature = "zip", feature = "xz"))]
use crate::ContainerError;
use crate::Result;
use std::io::{Read, Seek, Write};

/// Hard cap on the total decompressed size a codec will materialize from one archive.
/// Defends against decompression bombs at the cost of refusing legitimately huge archives.
#[cfg(any(feature = "zip", feature = "xz"))]
pub(crate) const MAX_DECOMPRESSED_SIZE: u64 = 4 * 1024 * 1024 * 1024; // 4GB

/// Fold one entry's declared `size` into the running `total` and return it as a `usize` for `write_sized`.
/// Both codecs route entries through here so the decompression-bomb cap and 32-bit-safe conversion live in
/// one place. `write_sized` pre-allocates the declared size, so an over-large one is rejected before that.
#[cfg(any(feature = "zip", feature = "xz"))]
pub(crate) fn checked_entry_size(total: &mut u64, size: u64) -> Result<usize> {
	*total = total.saturating_add(size);
	if *total > MAX_DECOMPRESSED_SIZE {
		return Err(ContainerError::SizeLimitExceeded {
			declared: *total,
			limit: MAX_DECOMPRESSED_SIZE,
		});
	}

	// `usize` is 32-bit on wasm, so convert fallibly to rule out a silent truncation into a smaller allocation.
	usize::try_from(size).map_err(|_| ContainerError::SizeLimitExceeded {
		declared: size,
		limit: usize::MAX as u64,
	})
}

#[cfg(feature = "zip")]
mod zip;
#[cfg(feature = "zip")]
pub use zip::{Zip, ZipWriter};

#[cfg(feature = "xz")]
mod xz;
#[cfg(feature = "xz")]
pub use xz::{Xz, XzWriter};

/// Streaming archive codec. The associated `Writer` type wraps a `Write + Seek` sink (zip needs
/// `Seek` for the central directory; xz doesn't but `Seek` is free on file-like sinks) and
/// accepts entries one at a time. `finish` flushes the codec's trailer and consumes the wrapper.
pub trait Archive {
	type Writer<W: Write + Seek>: ArchiveWriter
	where
		W: Write + Seek;

	fn writer<W: Write + Seek>(output: W) -> Result<Self::Writer<W>>;

	/// Read entries from `source` and write each into `dest`, streaming so neither the full
	/// archive nor the full container ever sits in memory at once.
	fn open<R: Read + Seek, C: AsyncContainer>(source: R, dest: &mut C) -> Result<()>;
}

pub trait ArchiveWriter: Sized {
	type Sink;

	fn write_entry(&mut self, path: &str, bytes: &[u8]) -> Result<()>;

	/// Finish the archive and return the underlying sink, for in-memory archives where the caller
	/// wants the written bytes (e.g. `Cursor<Vec<u8>>`) back.
	fn finish_into(self) -> Result<Self::Sink>;

	/// Finish the archive, discarding the sink. For file-backed archives the bytes are already on disk.
	fn finish(self) -> Result<()> {
		self.finish_into()?;
		Ok(())
	}
}

/// Archive container formats distinguishable by their leading magic bytes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ArchiveFormat {
	Xz,
	Zip,
}

impl ArchiveFormat {
	/// Sniff the format from the leading magic bytes: xz streams start with `FD 37 7A 58 5A 00`,
	/// zip archives with `50 4B 03 04` (`PK\x03\x04`). Returns `None` for anything else.
	pub fn detect(bytes: &[u8]) -> Option<Self> {
		if bytes.starts_with(&[0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00]) {
			Some(Self::Xz)
		} else if bytes.starts_with(&[0x50, 0x4B, 0x03, 0x04]) {
			Some(Self::Zip)
		} else {
			None
		}
	}
}

/// Deserialize an archive into `dest`, auto-detecting the format from `bytes`' magic header.
/// Errors if the bytes are neither a recognized xz nor zip archive.
#[cfg(any(feature = "xz", feature = "zip"))]
pub fn open_auto<C: AsyncContainer>(bytes: &[u8], dest: &mut C) -> Result<()> {
	let source = std::io::Cursor::new(bytes);
	match ArchiveFormat::detect(bytes) {
		#[cfg(feature = "xz")]
		Some(ArchiveFormat::Xz) => Xz::open(source, dest),
		#[cfg(feature = "zip")]
		Some(ArchiveFormat::Zip) => Zip::open(source, dest),
		#[allow(unreachable_patterns)]
		Some(format) => Err(ContainerError::Codec(format!("tried to open {format:?}, but the binary was compiled without the feature enabled  "))),
		None => Err(ContainerError::Codec("unrecognized archive format (not xz or zip) ".into())),
	}
}
