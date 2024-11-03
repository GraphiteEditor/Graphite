use std::io::{Error, ErrorKind, Read, Result, Seek, SeekFrom};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Endian {
	Little,
	Big,
}

pub struct TiffRead<R: Read + Seek> {
	reader: R,
	endian: Endian,
}

impl<R: Read + Seek> TiffRead<R> {
	pub fn new(mut reader: R) -> Result<Self> {
		let error = Error::new(ErrorKind::InvalidData, "Invalid Tiff format");

		let mut data = [0_u8; 2];
		reader.read_exact(&mut data)?;
		let endian = if data[0] == 0x49 && data[1] == 0x49 {
			Endian::Little
		} else if data[0] == 0x4d && data[1] == 0x4d {
			Endian::Big
		} else {
			return Err(error);
		};

		reader.read_exact(&mut data)?;
		let magic_number = match endian {
			Endian::Little => u16::from_le_bytes(data),
			Endian::Big => u16::from_be_bytes(data),
		};
		if magic_number != 42 {
			return Err(error);
		}

		Ok(Self { reader, endian })
	}

	pub fn endian(&self) -> Endian {
		self.endian
	}
}

impl<R: Read + Seek> Read for TiffRead<R> {
	fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
		self.reader.read(buf)
	}
}

impl<R: Read + Seek> Seek for TiffRead<R> {
	fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
		self.reader.seek(pos)
	}
}

impl<R: Read + Seek> TiffRead<R> {
	pub fn seek_from_start(&mut self, offset: u32) -> Result<u64> {
		self.reader.seek(SeekFrom::Start(offset.into()))
	}

	pub fn read_ascii(&mut self) -> Result<char> {
		let data = self.read_n::<1>()?;
		Ok(data[0] as char)
	}

	pub fn read_n<const N: usize>(&mut self) -> Result<[u8; N]> {
		let mut data = [0_u8; N];
		self.read_exact(&mut data)?;
		Ok(data)
	}

	pub fn read_u8(&mut self) -> Result<u8> {
		let data = self.read_n()?;
		match self.endian {
			Endian::Little => Ok(u8::from_le_bytes(data)),
			Endian::Big => Ok(u8::from_be_bytes(data)),
		}
	}

	pub fn read_u16(&mut self) -> Result<u16> {
		let data = self.read_n()?;
		match self.endian {
			Endian::Little => Ok(u16::from_le_bytes(data)),
			Endian::Big => Ok(u16::from_be_bytes(data)),
		}
	}

	pub fn read_u32(&mut self) -> Result<u32> {
		let data = self.read_n()?;
		match self.endian {
			Endian::Little => Ok(u32::from_le_bytes(data)),
			Endian::Big => Ok(u32::from_be_bytes(data)),
		}
	}

	pub fn read_u64(&mut self) -> Result<u64> {
		let data = self.read_n()?;
		match self.endian {
			Endian::Little => Ok(u64::from_le_bytes(data)),
			Endian::Big => Ok(u64::from_be_bytes(data)),
		}
	}

	pub fn read_i8(&mut self) -> Result<i8> {
		let data = self.read_n()?;
		match self.endian {
			Endian::Little => Ok(i8::from_le_bytes(data)),
			Endian::Big => Ok(i8::from_be_bytes(data)),
		}
	}

	pub fn read_i16(&mut self) -> Result<i16> {
		let data = self.read_n()?;
		match self.endian {
			Endian::Little => Ok(i16::from_le_bytes(data)),
			Endian::Big => Ok(i16::from_be_bytes(data)),
		}
	}

	pub fn read_i32(&mut self) -> Result<i32> {
		let data = self.read_n()?;
		match self.endian {
			Endian::Little => Ok(i32::from_le_bytes(data)),
			Endian::Big => Ok(i32::from_be_bytes(data)),
		}
	}

	pub fn read_i64(&mut self) -> Result<i64> {
		let data = self.read_n()?;
		match self.endian {
			Endian::Little => Ok(i64::from_le_bytes(data)),
			Endian::Big => Ok(i64::from_be_bytes(data)),
		}
	}

	pub fn read_f32(&mut self) -> Result<f32> {
		let data = self.read_n()?;
		match self.endian {
			Endian::Little => Ok(f32::from_le_bytes(data)),
			Endian::Big => Ok(f32::from_be_bytes(data)),
		}
	}

	pub fn read_f64(&mut self) -> Result<f64> {
		let data = self.read_n()?;
		match self.endian {
			Endian::Little => Ok(f64::from_le_bytes(data)),
			Endian::Big => Ok(f64::from_be_bytes(data)),
		}
	}
}
