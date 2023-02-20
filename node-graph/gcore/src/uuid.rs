use core::{
	fmt::{Display, Write},
	ops::{AddAssign, Deref},
	str::FromStr,
};
use dyn_any::{DynAny, StaticType};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, Deserialize, specta::Type, PartialEq, Eq, Hash, Default, DynAny, Ord, PartialOrd)]
pub struct Uuid(
	#[serde(with = "u64_string")]
	#[specta(type = String)]
	pub(crate) u64,
);

impl core::fmt::Debug for Uuid {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.write_str(self.0.to_string().as_str()).unwrap();
		Ok(())
	}
}

impl Display for Uuid {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.write_str(self.0.to_string().as_str()).unwrap();
		Ok(())
	}
}

impl Deref for Uuid {
	type Target = u64;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl From<u64> for Uuid {
	fn from(value: u64) -> Self {
		Uuid(value)
	}
}
impl From<Uuid> for u64 {
	fn from(value: Uuid) -> u64 {
		*value
	}
}

impl AddAssign for Uuid {
	fn add_assign(&mut self, rhs: Self) {
		self.0 += rhs.0
	}
}
impl AddAssign<u64> for Uuid {
	fn add_assign(&mut self, rhs: u64) {
		self.0 += rhs
	}
}

impl FromStr for Uuid {
	type Err = <u64 as FromStr>::Err;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		u64::from_str(s).map(|x| x.into())
	}
}

impl AsRef<u64> for Uuid {
	fn as_ref(&self) -> &u64 {
		&self.0
	}
}

mod u64_string {
	use serde::{self, Deserialize, Deserializer, Serializer};
	use std::str::FromStr;

	// The signature of a serialize_with function must follow the pattern:
	//
	//    fn serialize<S>(&T, S) -> Result<S::Ok, S::Error>
	//    where
	//        S: Serializer
	//
	// although it may also be generic over the input types T.
	pub fn serialize<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		serializer.serialize_str(&value.to_string())
	}

	// The signature of a deserialize_with function must follow the pattern:
	//
	//    fn deserialize<'de, D>(D) -> Result<T, D::Error>
	//    where
	//        D: Deserializer<'de>
	//
	// although it may also be generic over the output types T.
	pub fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
	where
		D: Deserializer<'de>,
	{
		let s = String::deserialize(deserializer)?;
		u64::from_str(&s).map_err(serde::de::Error::custom)
	}
}
