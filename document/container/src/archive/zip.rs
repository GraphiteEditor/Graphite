//! Zip archive codec.

use crate::archive::{Archive, ArchiveWriter, checked_entry_size};
use crate::{Container, ContainerError, Result, validate_path};
use std::io::{Read, Seek, Write};

use zip::ZipArchive;
use zip::write::{SimpleFileOptions, ZipWriter as InnerZipWriter};

pub struct Zip;

pub struct ZipWriter<W: Write + Seek> {
	inner: InnerZipWriter<W>,
	options: SimpleFileOptions,
}

impl Archive for Zip {
	type Writer<W: Write + Seek> = ZipWriter<W>;

	fn writer<W: Write + Seek>(output: W) -> Result<Self::Writer<W>> {
		Ok(ZipWriter {
			inner: InnerZipWriter::new(output),
			options: SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
		})
	}

	fn open<R: Read + Seek, C: Container>(source: R, dest: &mut C) -> Result<()> {
		let mut archive = ZipArchive::new(source).map_err(zip_err)?;

		// Zip headers declare each entry's uncompressed size up front; `checked_entry_size` caps the running
		// total so a malicious archive can't exhaust memory or disk before any bytes are read.
		let mut total_size = 0u64;

		for index in 0..archive.len() {
			let mut entry = archive.by_index(index).map_err(zip_err)?;
			if !entry.is_file() {
				continue;
			}
			let name = entry.name().to_string();
			validate_path(&name)?;

			let size = checked_entry_size(&mut total_size, entry.size())?;

			dest.write_sized(&name, size, &mut |buffer| {
				entry.read_exact(buffer).map_err(ContainerError::Io)?;
				Ok(())
			})?;
		}

		Ok(())
	}
}

impl<W: Write + Seek> ArchiveWriter for ZipWriter<W> {
	fn write_entry(&mut self, path: &str, bytes: &[u8]) -> Result<()> {
		validate_path(path)?;
		self.inner.start_file(path, self.options).map_err(zip_err)?;
		self.inner.write_all(bytes)?;
		Ok(())
	}

	fn finish(self) -> Result<()> {
		self.inner.finish().map_err(zip_err)?;
		Ok(())
	}
}

impl<W: Write + Seek> ZipWriter<W> {
	/// Finish the archive and return the underlying sink, for in-memory archives where the caller
	/// wants the written bytes (e.g. `Cursor<Vec<u8>>`) back.
	pub fn finish_into(self) -> Result<W> {
		self.inner.finish().map_err(zip_err)
	}
}

fn zip_err(error: zip::result::ZipError) -> ContainerError {
	// Preserve a real I/O failure (disk full, etc.) as a structured `Io` so callers can tell it apart from a
	// corrupt-archive `Codec` error; only the genuinely archive-format errors collapse into `Codec`.
	match error {
		zip::result::ZipError::Io(io) => ContainerError::Io(io),
		other => ContainerError::Codec(format!("zip: {other}")),
	}
}
