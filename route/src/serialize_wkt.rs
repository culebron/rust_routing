use serde::{self, Deserialize, Serializer, Deserializer};
use geo::LineString;
use wkt::{ToWkt, TryFromWkt};

pub fn serialize<S>(
	g: &LineString,
	serializer: S,
) -> Result<S::Ok, S::Error>
where
	S: Serializer,
{
	let s = g.to_wkt().to_string();
	serializer.serialize_str(&s)
}

pub fn deserialize<'de, D>(
	deserializer: D,
) -> Result<LineString, D::Error>
where
	D: Deserializer<'de>,
{
	let s = String::deserialize(deserializer)?;
	LineString::try_from_wkt_str(&s).map_err(serde::de::Error::custom)
}
