use crate::shaders::buffer_struct::BufferStruct;

macro_rules! glam_array {
	($t:ty, $a:ty) => {
		unsafe impl BufferStruct for $t {
			type Buffer = $a;

			#[inline]
			fn write(from: Self) -> Self::Buffer {
				<$t>::to_array(&from)
			}

			#[inline]
			fn read(from: Self::Buffer) -> Self {
				<$t>::from_array(from)
			}
		}
	};
}

macro_rules! glam_cols_array {
	($t:ty, $a:ty) => {
		unsafe impl BufferStruct for $t {
			type Buffer = $a;

			#[inline]
			fn write(from: Self) -> Self::Buffer {
				<$t>::to_cols_array(&from)
			}

			#[inline]
			fn read(from: Self::Buffer) -> Self {
				<$t>::from_cols_array(&from)
			}
		}
	};
}

glam_array!(glam::Vec2, [f32; 2]);
glam_array!(glam::Vec3, [f32; 3]);
// glam_array!(Vec3A, [f32; 4]);
glam_array!(glam::Vec4, [f32; 4]);
glam_array!(glam::Quat, [f32; 4]);
glam_cols_array!(glam::Mat2, [f32; 4]);
glam_cols_array!(glam::Mat3, [f32; 9]);
// glam_cols_array!(Mat3A, [f32; 4]);
glam_cols_array!(glam::Mat4, [f32; 16]);
glam_cols_array!(glam::Affine2, [f32; 6]);
glam_cols_array!(glam::Affine3A, [f32; 12]);

glam_array!(glam::DVec2, [f64; 2]);
glam_array!(glam::DVec3, [f64; 3]);
glam_array!(glam::DVec4, [f64; 4]);
glam_array!(glam::DQuat, [f64; 4]);
glam_cols_array!(glam::DMat2, [f64; 4]);
glam_cols_array!(glam::DMat3, [f64; 9]);
glam_cols_array!(glam::DMat4, [f64; 16]);
glam_cols_array!(glam::DAffine2, [f64; 6]);
glam_cols_array!(glam::DAffine3, [f64; 12]);

glam_array!(glam::I16Vec2, [i16; 2]);
glam_array!(glam::I16Vec3, [i16; 3]);
glam_array!(glam::I16Vec4, [i16; 4]);

glam_array!(glam::U16Vec2, [u16; 2]);
glam_array!(glam::U16Vec3, [u16; 3]);
glam_array!(glam::U16Vec4, [u16; 4]);

glam_array!(glam::IVec2, [i32; 2]);
glam_array!(glam::IVec3, [i32; 3]);
glam_array!(glam::IVec4, [i32; 4]);

glam_array!(glam::UVec2, [u32; 2]);
glam_array!(glam::UVec3, [u32; 3]);
glam_array!(glam::UVec4, [u32; 4]);

glam_array!(glam::I64Vec2, [i64; 2]);
glam_array!(glam::I64Vec3, [i64; 3]);
glam_array!(glam::I64Vec4, [i64; 4]);

glam_array!(glam::U64Vec2, [u64; 2]);
glam_array!(glam::U64Vec3, [u64; 3]);
glam_array!(glam::U64Vec4, [u64; 4]);

unsafe impl BufferStruct for glam::Vec3A {
	type Buffer = [f32; 4];

	#[inline]
	fn write(from: Self) -> Self::Buffer {
		glam::Vec4::to_array(&from.extend(0.))
	}

	#[inline]
	fn read(from: Self::Buffer) -> Self {
		glam::Vec3A::from_vec4(glam::Vec4::from_array(from))
	}
}

/// do NOT use slices, otherwise spirv will fail to compile
unsafe impl BufferStruct for glam::Mat3A {
	type Buffer = [f32; 12];

	#[inline]
	fn write(from: Self) -> Self::Buffer {
		let a = from.to_cols_array();
		[a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7], a[8], 0., 0., 0.]
	}

	#[inline]
	fn read(from: Self::Buffer) -> Self {
		let a = from;
		glam::Mat3A::from_cols_array(&[a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7], a[8]])
	}
}
