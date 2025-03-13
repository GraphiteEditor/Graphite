use super::file::TiffRead;
use super::values::{CurveLookupTable, Rational, Transform};
use super::{Ifd, IfdTagType, TiffError};
use std::io::{Read, Seek};

pub struct TypeAscii;
pub struct TypeByte;
pub struct TypeShort;
pub struct TypeLong;
pub struct TypeRational;
pub struct TypeSByte;
pub struct TypeSShort;
pub struct TypeSLong;
pub struct TypeSRational;
pub struct TypeFloat;
pub struct TypeDouble;
pub struct TypeUndefined;

pub struct TypeNumber;
pub struct TypeSNumber;
pub struct TypeIfd;

pub trait PrimitiveType {
	type Output;

	fn get_size(the_type: IfdTagType) -> Option<u32>;

	fn read_primitive<R: Read + Seek>(the_type: IfdTagType, file: &mut TiffRead<R>) -> Result<Self::Output, TiffError>;
}

impl PrimitiveType for TypeAscii {
	type Output = char;

	fn get_size(the_type: IfdTagType) -> Option<u32> {
		match the_type {
			IfdTagType::Ascii => Some(1),
			_ => None,
		}
	}

	fn read_primitive<R: Read + Seek>(_: IfdTagType, file: &mut TiffRead<R>) -> Result<Self::Output, TiffError> {
		let value = file.read_ascii()?;
		if value.is_ascii() { Ok(value) } else { Err(TiffError::InvalidValue) }
	}
}

impl PrimitiveType for TypeByte {
	type Output = u8;

	fn get_size(the_type: IfdTagType) -> Option<u32> {
		match the_type {
			IfdTagType::Byte => Some(1),
			_ => None,
		}
	}

	fn read_primitive<R: Read + Seek>(_: IfdTagType, file: &mut TiffRead<R>) -> Result<Self::Output, TiffError> {
		Ok(file.read_u8()?)
	}
}

impl PrimitiveType for TypeShort {
	type Output = u16;

	fn get_size(the_type: IfdTagType) -> Option<u32> {
		match the_type {
			IfdTagType::Short => Some(2),
			_ => None,
		}
	}

	fn read_primitive<R: Read + Seek>(_: IfdTagType, file: &mut TiffRead<R>) -> Result<Self::Output, TiffError> {
		Ok(file.read_u16()?)
	}
}

impl PrimitiveType for TypeLong {
	type Output = u32;

	fn get_size(the_type: IfdTagType) -> Option<u32> {
		match the_type {
			IfdTagType::Long => Some(4),
			_ => None,
		}
	}

	fn read_primitive<R: Read + Seek>(_: IfdTagType, file: &mut TiffRead<R>) -> Result<Self::Output, TiffError> {
		Ok(file.read_u32()?)
	}
}

impl PrimitiveType for TypeRational {
	type Output = Rational<u32>;

	fn get_size(the_type: IfdTagType) -> Option<u32> {
		match the_type {
			IfdTagType::Rational => Some(8),
			_ => None,
		}
	}

	fn read_primitive<R: Read + Seek>(the_type: IfdTagType, file: &mut TiffRead<R>) -> Result<Self::Output, TiffError> {
		let numerator = TypeLong::read_primitive(the_type, file)?;
		let denominator = TypeLong::read_primitive(the_type, file)?;

		Ok(Rational { numerator, denominator })
	}
}

impl PrimitiveType for TypeSByte {
	type Output = i8;

	fn get_size(the_type: IfdTagType) -> Option<u32> {
		match the_type {
			IfdTagType::SByte => Some(1),
			_ => None,
		}
	}

	fn read_primitive<R: Read + Seek>(_: IfdTagType, file: &mut TiffRead<R>) -> Result<Self::Output, TiffError> {
		Ok(file.read_i8()?)
	}
}

impl PrimitiveType for TypeSShort {
	type Output = i16;

	fn get_size(the_type: IfdTagType) -> Option<u32> {
		match the_type {
			IfdTagType::SShort => Some(2),
			_ => None,
		}
	}

	fn read_primitive<R: Read + Seek>(_: IfdTagType, file: &mut TiffRead<R>) -> Result<Self::Output, TiffError> {
		Ok(file.read_i16()?)
	}
}

impl PrimitiveType for TypeSLong {
	type Output = i32;

	fn get_size(the_type: IfdTagType) -> Option<u32> {
		match the_type {
			IfdTagType::SLong => Some(4),
			_ => None,
		}
	}

	fn read_primitive<R: Read + Seek>(_: IfdTagType, file: &mut TiffRead<R>) -> Result<Self::Output, TiffError> {
		Ok(file.read_i32()?)
	}
}

impl PrimitiveType for TypeSRational {
	type Output = Rational<i32>;

	fn get_size(the_type: IfdTagType) -> Option<u32> {
		match the_type {
			IfdTagType::SRational => Some(8),
			_ => None,
		}
	}

	fn read_primitive<R: Read + Seek>(the_type: IfdTagType, file: &mut TiffRead<R>) -> Result<Self::Output, TiffError> {
		let numerator = TypeSLong::read_primitive(the_type, file)?;
		let denominator = TypeSLong::read_primitive(the_type, file)?;

		Ok(Rational { numerator, denominator })
	}
}

impl PrimitiveType for TypeFloat {
	type Output = f32;

	fn get_size(the_type: IfdTagType) -> Option<u32> {
		match the_type {
			IfdTagType::Float => Some(4),
			_ => None,
		}
	}

	fn read_primitive<R: Read + Seek>(_: IfdTagType, file: &mut TiffRead<R>) -> Result<Self::Output, TiffError> {
		Ok(file.read_f32()?)
	}
}

impl PrimitiveType for TypeDouble {
	type Output = f64;

	fn get_size(the_type: IfdTagType) -> Option<u32> {
		match the_type {
			IfdTagType::Double => Some(8),
			_ => None,
		}
	}

	fn read_primitive<R: Read + Seek>(_: IfdTagType, file: &mut TiffRead<R>) -> Result<Self::Output, TiffError> {
		Ok(file.read_f64()?)
	}
}

impl PrimitiveType for TypeUndefined {
	type Output = ();

	fn get_size(_: IfdTagType) -> Option<u32> {
		todo!()
	}

	fn read_primitive<R: Read + Seek>(_: IfdTagType, _: &mut TiffRead<R>) -> Result<Self::Output, TiffError> {
		todo!()
	}
}

impl PrimitiveType for TypeNumber {
	type Output = u32;

	fn get_size(the_type: IfdTagType) -> Option<u32> {
		match the_type {
			IfdTagType::Byte => TypeByte::get_size(the_type),
			IfdTagType::Short => TypeShort::get_size(the_type),
			IfdTagType::Long => TypeLong::get_size(the_type),
			_ => None,
		}
	}

	fn read_primitive<R: Read + Seek>(the_type: IfdTagType, file: &mut TiffRead<R>) -> Result<Self::Output, TiffError> {
		Ok(match the_type {
			IfdTagType::Byte => TypeByte::read_primitive(the_type, file)?.into(),
			IfdTagType::Short => TypeShort::read_primitive(the_type, file)?.into(),
			IfdTagType::Long => TypeLong::read_primitive(the_type, file)?,
			_ => unreachable!(),
		})
	}
}

impl PrimitiveType for TypeSNumber {
	type Output = i32;

	fn get_size(the_type: IfdTagType) -> Option<u32> {
		match the_type {
			IfdTagType::SByte => TypeSByte::get_size(the_type),
			IfdTagType::SShort => TypeSShort::get_size(the_type),
			IfdTagType::SLong => TypeSLong::get_size(the_type),
			_ => None,
		}
	}

	fn read_primitive<R: Read + Seek>(the_type: IfdTagType, file: &mut TiffRead<R>) -> Result<Self::Output, TiffError> {
		Ok(match the_type {
			IfdTagType::SByte => TypeSByte::read_primitive(the_type, file)?.into(),
			IfdTagType::SShort => TypeSShort::read_primitive(the_type, file)?.into(),
			IfdTagType::SLong => TypeSLong::read_primitive(the_type, file)?,
			_ => unreachable!(),
		})
	}
}

impl PrimitiveType for TypeIfd {
	type Output = Ifd;

	fn get_size(the_type: IfdTagType) -> Option<u32> {
		TypeLong::get_size(the_type)
	}

	fn read_primitive<R: Read + Seek>(the_type: IfdTagType, file: &mut TiffRead<R>) -> Result<Self::Output, TiffError> {
		let offset = TypeLong::read_primitive(the_type, file)?;
		Ifd::new_from_offset(file, offset)
	}
}

pub trait TagType {
	type Output;

	fn read<R: Read + Seek>(file: &mut TiffRead<R>) -> Result<Self::Output, TiffError>;
}

impl<T: PrimitiveType> TagType for T {
	type Output = T::Output;

	fn read<R: Read + Seek>(file: &mut TiffRead<R>) -> Result<Self::Output, TiffError> {
		let the_type = IfdTagType::from(file.read_u16()?);
		let count = file.read_u32()?;

		if count != 1 {
			return Err(TiffError::InvalidCount);
		}

		let size = T::get_size(the_type).ok_or(TiffError::InvalidType)?;
		if count * size > 4 {
			let offset = file.read_u32()?;
			file.seek_from_start(offset)?;
		}

		T::read_primitive(the_type, file)
	}
}

pub struct Array<T: PrimitiveType> {
	primitive_type: std::marker::PhantomData<T>,
}

pub struct ConstArray<T: PrimitiveType, const N: usize> {
	primitive_type: std::marker::PhantomData<T>,
}

impl<T: PrimitiveType> TagType for Array<T> {
	type Output = Vec<T::Output>;

	fn read<R: Read + Seek>(file: &mut TiffRead<R>) -> Result<Self::Output, TiffError> {
		let the_type = IfdTagType::from(file.read_u16()?);
		let count = file.read_u32()?;

		let size = T::get_size(the_type).ok_or(TiffError::InvalidType)?;
		if count * size > 4 {
			let offset = file.read_u32()?;
			file.seek_from_start(offset)?;
		}

		let mut ans = Vec::with_capacity(count.try_into()?);
		for _ in 0..count {
			ans.push(T::read_primitive(the_type, file)?);
		}
		Ok(ans)
	}
}

impl<T: PrimitiveType, const N: usize> TagType for ConstArray<T, N> {
	type Output = [T::Output; N];

	fn read<R: Read + Seek>(file: &mut TiffRead<R>) -> Result<Self::Output, TiffError> {
		let the_type = IfdTagType::from(file.read_u16()?);
		let count = file.read_u32()?;

		if count != N.try_into()? {
			return Err(TiffError::InvalidCount);
		}

		let size = T::get_size(the_type).ok_or(TiffError::InvalidType)?;
		if count * size > 4 {
			let offset = file.read_u32()?;
			file.seek_from_start(offset)?;
		}

		let mut ans = Vec::with_capacity(count.try_into()?);
		for _ in 0..count {
			ans.push(T::read_primitive(the_type, file)?);
		}
		ans.try_into().map_err(|_| TiffError::InvalidCount)
	}
}

pub struct TypeString;
pub struct TypeSonyToneCurve;
pub struct TypeOrientation;

impl TagType for TypeString {
	type Output = String;

	fn read<R: Read + Seek>(file: &mut TiffRead<R>) -> Result<Self::Output, TiffError> {
		let string = Array::<TypeAscii>::read(file)?;

		// Skip the NUL character at the end
		let len = string.len();
		Ok(string.into_iter().take(len - 1).collect())
	}
}

impl TagType for TypeSonyToneCurve {
	type Output = CurveLookupTable;

	fn read<R: Read + Seek>(file: &mut TiffRead<R>) -> Result<Self::Output, TiffError> {
		let values = ConstArray::<TypeShort, 4>::read(file)?;
		Ok(CurveLookupTable::from_sony_tone_table(values))
	}
}

impl TagType for TypeOrientation {
	type Output = Transform;

	fn read<R: Read + Seek>(file: &mut TiffRead<R>) -> Result<Self::Output, TiffError> {
		Ok(match TypeShort::read(file)? {
			1 => Transform::Horizontal,
			2 => Transform::MirrorHorizontal,
			3 => Transform::Rotate180,
			4 => Transform::MirrorVertical,
			5 => Transform::MirrorHorizontalRotate270,
			6 => Transform::Rotate90,
			7 => Transform::MirrorHorizontalRotate90,
			8 => Transform::Rotate270,
			_ => return Err(TiffError::InvalidValue),
		})
	}
}
