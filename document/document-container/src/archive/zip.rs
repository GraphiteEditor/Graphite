//! Zip archive codec.

use crate::archive::{Archive, ArchiveWriter, MAX_DECOMPRESSED_SIZE};
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

	fn deserialize<R: Read + Seek, C: Container>(source: R, dest: &mut C) -> Result<()> {
		let mut archive = ZipArchive::new(source).map_err(zip_err)?;

		// Zip headers declare each entry's uncompressed size up front, and `write_sized` pre-allocates
		// from it before reading bytes. Cap the running total so a malicious archive can't exhaust memory
		// or disk by claiming a huge size (the xz codec caps its declared total the same way).
		let mut total_size = 0u64;

		for index in 0..archive.len() {
			let mut entry = archive.by_index(index).map_err(zip_err)?;
			if !entry.is_file() {
				continue;
			}
			let name = entry.name().to_string();
			validate_path(&name)?;

			let size = entry.size();
			total_size = total_size.saturating_add(size);
			if total_size >= MAX_DECOMPRESSED_SIZE {
				return Err(ContainerError::SizeLimitExceeded {
					declared: total_size,
					limit: MAX_DECOMPRESSED_SIZE,
				});
			}

			dest.write_sized(&name, size as usize, &mut |buffer| {
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

fn zip_err(error: zip::result::ZipError) -> ContainerError {
	ContainerError::Codec(format!("zip: {error}"))
}
