use std::io::{Read, Seek};
use thiserror::Error;

use super::file::TiffRead;
use super::values::Rational;
use super::IfdTagType;

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

pub trait PrimitiveType {
    type Output;

    fn get_size(type_: IfdTagType) -> Option<u32>;

    fn read_primitive<R: Read + Seek>(
        type_: IfdTagType,
        file: &mut TiffRead<R>,
    ) -> Result<Self::Output, DecoderError>;
}

impl PrimitiveType for TypeAscii {
    type Output = char;

    fn get_size(type_: IfdTagType) -> Option<u32> {
        match type_ {
            IfdTagType::Ascii => Some(1),
            _ => None,
        }
    }

    fn read_primitive<R: Read + Seek>(
        _: IfdTagType,
        file: &mut TiffRead<R>,
    ) -> Result<Self::Output, DecoderError> {
        let value = file.read_ascii()?;
        if value.is_ascii() {
            Ok(value)
        } else {
            Err(DecoderError::InvalidValue)
        }
    }
}

impl PrimitiveType for TypeByte {
    type Output = u8;

    fn get_size(type_: IfdTagType) -> Option<u32> {
        match type_ {
            IfdTagType::Byte => Some(1),
            _ => None,
        }
    }

    fn read_primitive<R: Read + Seek>(
        _: IfdTagType,
        file: &mut TiffRead<R>,
    ) -> Result<Self::Output, DecoderError> {
        Ok(file.read_u8()?)
    }
}

impl PrimitiveType for TypeShort {
    type Output = u16;

    fn get_size(type_: IfdTagType) -> Option<u32> {
        match type_ {
            IfdTagType::Short => Some(2),
            _ => None,
        }
    }

    fn read_primitive<R: Read + Seek>(
        _: IfdTagType,
        file: &mut TiffRead<R>,
    ) -> Result<Self::Output, DecoderError> {
        Ok(file.read_u16()?)
    }
}

impl PrimitiveType for TypeLong {
    type Output = u32;

    fn get_size(type_: IfdTagType) -> Option<u32> {
        match type_ {
            IfdTagType::Long => Some(4),
            _ => None,
        }
    }

    fn read_primitive<R: Read + Seek>(
        _: IfdTagType,
        file: &mut TiffRead<R>,
    ) -> Result<Self::Output, DecoderError> {
        Ok(file.read_u32()?)
    }
}

impl PrimitiveType for TypeRational {
    type Output = Rational<u32>;

    fn get_size(type_: IfdTagType) -> Option<u32> {
        match type_ {
            IfdTagType::Rational => Some(8),
            _ => None,
        }
    }

    fn read_primitive<R: Read + Seek>(
        type_: IfdTagType,
        file: &mut TiffRead<R>,
    ) -> Result<Self::Output, DecoderError> {
        let numerator = TypeLong::read_primitive(type_, file)?;
        let denominator = TypeLong::read_primitive(type_, file)?;
        
        Ok(Rational {
            numerator,
            denominator,
        })
    }
}

impl PrimitiveType for TypeSByte {
    type Output = i8;

    fn get_size(type_: IfdTagType) -> Option<u32> {
        match type_ {
            IfdTagType::SByte => Some(1),
            _ => None,
        }
    }

    fn read_primitive<R: Read + Seek>(
        _: IfdTagType,
        file: &mut TiffRead<R>,
    ) -> Result<Self::Output, DecoderError> {
        Ok(file.read_i8()?)
    }
}

impl PrimitiveType for TypeSShort {
    type Output = i16;

    fn get_size(type_: IfdTagType) -> Option<u32> {
        match type_ {
            IfdTagType::SShort => Some(2),
            _ => None,
        }
    }

    fn read_primitive<R: Read + Seek>(
        _: IfdTagType,
        file: &mut TiffRead<R>,
    ) -> Result<Self::Output, DecoderError> {
        Ok(file.read_i16()?)
    }
}

impl PrimitiveType for TypeSLong {
    type Output = i32;

    fn get_size(type_: IfdTagType) -> Option<u32> {
        match type_ {
            IfdTagType::SLong => Some(4),
            _ => None,
        }
    }

    fn read_primitive<R: Read + Seek>(
        _: IfdTagType,
        file: &mut TiffRead<R>,
    ) -> Result<Self::Output, DecoderError> {
        Ok(file.read_i32()?)
    }
}

impl PrimitiveType for TypeSRational {
    type Output = Rational<i32>;

    fn get_size(type_: IfdTagType) -> Option<u32> {
        match type_ {
            IfdTagType::SRational => Some(8),
            _ => None,
        }
    }

    fn read_primitive<R: Read + Seek>(
        type_: IfdTagType,
        file: &mut TiffRead<R>,
    ) -> Result<Self::Output, DecoderError> {
        let numerator = TypeSLong::read_primitive(type_, file)?;
        let denominator = TypeSLong::read_primitive(type_, file)?;
        
        Ok(Rational {
            numerator,
            denominator,
        })
    }
}

impl PrimitiveType for TypeFloat {
    type Output = f32;

    fn get_size(type_: IfdTagType) -> Option<u32> {
        match type_ {
            IfdTagType::Float => Some(4),
            _ => None,
        }
    }

    fn read_primitive<R: Read + Seek>(
        _: IfdTagType,
        file: &mut TiffRead<R>,
    ) -> Result<Self::Output, DecoderError> {
        Ok(file.read_f32()?)
    }
}

impl PrimitiveType for TypeDouble {
    type Output = f64;

    fn get_size(type_: IfdTagType) -> Option<u32> {
        match type_ {
            IfdTagType::Double => Some(8),
            _ => None,
        }
    }

    fn read_primitive<R: Read + Seek>(
        _: IfdTagType,
        file: &mut TiffRead<R>,
    ) -> Result<Self::Output, DecoderError> {
        Ok(file.read_f64()?)
    }
}

impl PrimitiveType for TypeUndefined {
    type Output = ();

    fn get_size(_: IfdTagType) -> Option<u32> {
        todo!()
    }

    fn read_primitive<R: Read + Seek>(
        _: IfdTagType,
        _: &mut TiffRead<R>,
    ) -> Result<Self::Output, DecoderError> {
        todo!()
    }
}

impl PrimitiveType for TypeNumber {
    type Output = u32;

    fn get_size(type_: IfdTagType) -> Option<u32> {
        match type_ {
            IfdTagType::Byte => TypeByte::get_size(type_),
            IfdTagType::Short => TypeShort::get_size(type_),
            IfdTagType::Long => TypeLong::get_size(type_),
            _ => None,
        }
    }

    fn read_primitive<R: Read + Seek>(
        type_: IfdTagType,
        file: &mut TiffRead<R>,
    ) -> Result<Self::Output, DecoderError> {
        Ok(match type_ {
            IfdTagType::Byte => TypeByte::read_primitive(type_, file)?.into(),
            IfdTagType::Short => TypeShort::read_primitive(type_, file)?.into(),
            IfdTagType::Long => TypeLong::read_primitive(type_, file)?,
            _ => unreachable!(),
        })
    }
}

impl PrimitiveType for TypeSNumber {
    type Output = i32;

    fn get_size(type_: IfdTagType) -> Option<u32> {
        match type_ {
            IfdTagType::SByte => TypeSByte::get_size(type_),
            IfdTagType::SShort => TypeSShort::get_size(type_),
            IfdTagType::SLong => TypeSLong::get_size(type_),
            _ => None,
        }
    }

    fn read_primitive<R: Read + Seek>(
        type_: IfdTagType,
        file: &mut TiffRead<R>,
    ) -> Result<Self::Output, DecoderError> {
        Ok(match type_ {
            IfdTagType::SByte => TypeSByte::read_primitive(type_, file)?.into(),
            IfdTagType::SShort => TypeSShort::read_primitive(type_, file)?.into(),
            IfdTagType::SLong => TypeSLong::read_primitive(type_, file)?,
            _ => unreachable!(),
        })
    }
}

pub trait TagType {
    type Output;

    fn read<R: Read+Seek>(file: &mut TiffRead<R>) -> Result<Self::Output, DecoderError>;
}

impl<T: PrimitiveType> TagType for T {
    type Output = T::Output;

    fn read<R: Read+Seek>(file: &mut TiffRead<R>) -> Result<Self::Output, DecoderError> {
        let type_ = IfdTagType::try_from(file.read_u16()?).map_err(|_| DecoderError::InvalidType)?;
        let count = file.read_u32()?;

        if count != 1 {
            return Err(DecoderError::InvalidCount)
        }
        
        let size = T::get_size(type_).ok_or(DecoderError::InvalidType)?;
        if count*size > 4 {
            let offset = file.read_u32()?;
            file.seek_from_start(offset)?;
        }

        T::read_primitive(type_, file)
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

    fn read<R: Read+Seek>(file: &mut TiffRead<R>) -> Result<Self::Output, DecoderError> {
        let type_ = IfdTagType::try_from(file.read_u16()?).map_err(|_| DecoderError::InvalidType)?;
        let count = file.read_u32()?;

        let size = T::get_size(type_).ok_or(DecoderError::InvalidType)?;
        if count*size > 4 {
            let offset = file.read_u32()?;
            file.seek_from_start(offset)?;
        }

        let mut ans = Vec::with_capacity(count.try_into()?);
        for _ in 0..count {
            ans.push(T::read_primitive(type_, file)?);
        }
        Ok(ans)
    }
}

impl<T: PrimitiveType, const N: usize> TagType for ConstArray<T, N> {
    type Output = [T::Output; N];

    fn read<R: Read+Seek>(file: &mut TiffRead<R>) -> Result<Self::Output, DecoderError> {
        let type_ = IfdTagType::try_from(file.read_u16()?).map_err(|_| DecoderError::InvalidType)?;
        let count = file.read_u32()?;
        
        if count != N.try_into()? {
            return Err(DecoderError::InvalidCount)
        }
        
        let size = T::get_size(type_).ok_or(DecoderError::InvalidType)?;
        if count*size > 4 {
            let offset = file.read_u32()?;
            file.seek_from_start(offset)?;
        }

        let mut ans = Vec::with_capacity(count.try_into()?);
        for _ in 0..count {
            ans.push(T::read_primitive(type_, file)?);
        }
        ans.try_into().map_err(|_| DecoderError::InvalidCount)
    }
}

#[derive(Error, Debug)]
pub enum DecoderError {
    #[error("The value was invalid")]
    InvalidValue,
    #[error("The type was invalid")]
    InvalidType,
    #[error("The count was invalid")]
    InvalidCount,
    #[error("The tag was invalid")]
    InvalidTag,
    #[error("An error occurred when converting integer from one type to another")]
    ConversionError(#[from] std::num::TryFromIntError),
    #[error("An IO Error ocurred")]
    IoError(#[from] std::io::Error),
}
