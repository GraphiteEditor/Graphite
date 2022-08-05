use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::iter::FromIterator;

/// Necessary because serde can't serialize hashmaps when the keys don't implement display.
pub fn serialize<'a, T, K, V, S>(target: T, ser: S) -> Result<S::Ok, S::Error>
where
	S: Serializer,
	T: IntoIterator<Item = (&'a K, &'a V)>,
	K: Serialize + 'a,
	V: Serialize + 'a,
{
	let container: Vec<_> = target.into_iter().collect();
	serde::Serialize::serialize(&container, ser)
}

pub fn deserialize<'de, T, K, V, D>(des: D) -> Result<T, D::Error>
where
	D: Deserializer<'de>,
	T: FromIterator<(K, V)>,
	K: Deserialize<'de>,
	V: Deserialize<'de>,
{
	let container: Vec<_> = serde::Deserialize::deserialize(des)?;
	Ok(T::from_iter(container.into_iter()))
}
