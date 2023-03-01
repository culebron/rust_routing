use quick_xml::events::BytesStart;
use serde::{Serialize, Deserialize};
use std::{
	collections::HashMap,
	fmt::Display,
	sync::Arc,
};

use crate::errors::ReadError;

// not yet used in Node struct. Used for safety in merge/graph readers
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeId(pub i64);
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct WayId(pub i64);
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelId(pub i64);

/// An OpenStreetMap object.
#[derive(Debug, Clone)]
pub enum OsmObj {
	/// A node
	Node(Node),
	/// A way
	Way(Way),
	// /// A relation
	Relation(Relation),
}

impl OsmObj {
	pub fn tags_insert(&mut self, k: Arc<str>, v: Arc<str>) {
		match self {
			OsmObj::Node(n) => { n.tags.insert(k, v); }
			OsmObj::Way(n) => { n.tags.insert(k, v); }
			OsmObj::Relation(n) => { n.tags.insert(k, v); }
		}
	}
}

pub type Tags = HashMap<Arc<str>, Arc<str>>;

#[derive(Debug, Clone)]
pub struct OsmElementAttrs {
	pub id: i64,
	pub timestamp: Option<Arc<str>>,
	pub uid: Option<i64>,
	pub user: Option<Arc<str>>,
	pub visible: Option<bool>,
	pub deleted: Option<bool>,
	pub version: Option<u32>,
	pub changeset: Option<u64>,
}


impl OsmElementAttrs {
	pub fn new() -> Self {
		Self {
			id: -1,
			timestamp: None,
			uid: Some(1),
			user: Some("nobody".into()),
			visible: Some(true),
			deleted: Some(false),
			version: Some(1),
			changeset: Some(1),
		}
	}

	fn _do_push<T: Display>(&self, elt: &mut BytesStart, key: &str, val: T) {
		elt.push_attribute((key, &val.to_string() as &str));
	}

	pub fn push_to(&self, elt: &mut BytesStart) {
		self._do_push(elt, "id", self.id);
		self._do_push(elt, "timestamp", self.timestamp.as_ref().unwrap_or(&Arc::from("2023-01-01T00:00:00Z")));
		//self._do_push(elt, "uid", self.uid.unwrap_or(1));
		//self.user.as_ref().map(|u| self._do_push(elt, "user", u));
		self.visible.map(|v| self._do_push(elt, "visible", v));
		self.deleted.map(|d| self._do_push(elt, "deleted", d));
		self._do_push(elt, "version", self.version.unwrap_or(1));
		self.changeset.map(|c| self._do_push(elt, "changeset", c));
	}
}

//#[derive(Debug)]
pub type ParsedAttrs = HashMap<Arc<str>, Arc<str>>;

impl TryFrom<&ParsedAttrs> for OsmElementAttrs {
	type Error = ReadError;
	fn try_from(attrs: &ParsedAttrs) -> Result<Self, Self::Error> {

		Ok(Self {
			id: 		attrs.get("id")			.map(|v| v.parse::<i64>())	.transpose()?.unwrap_or(-1),
			changeset: 	attrs.get("changeset")	.map(|v| v.parse::<u64>())	.transpose()?,
			deleted: 	attrs.get("deleted")	.map(|v| v.parse::<bool>())	.transpose()?,
			timestamp: 	attrs.get("timestamp")	.map(|v| v.clone()),
			uid: 		attrs.get("uid")		.map(|v| v.parse::<i64>())	.transpose()?,
			user: 		attrs.get("user")		.map(|v| v.clone()),
			version: 	attrs.get("version")	.map(|v| v.parse::<u32>())	.transpose()?,
			visible: 	attrs.get("visible")	.map(|v| v.parse::<bool>())	.transpose()?,
		})
	}
}



#[derive(Debug, Clone)]
pub struct Way {
	pub attrs: OsmElementAttrs,
	pub nodes: Vec<i64>,
	pub tags: Tags,
}

#[derive(Debug, Clone)]
pub struct Node {
	pub attrs: OsmElementAttrs,
	pub lat: f32,
	pub lon: f32,
	pub tags: Tags
}

#[derive(Debug, Clone)]
pub struct Relation {
	pub attrs: OsmElementAttrs,
	pub tags: Tags,
	pub members: Vec<Member>,
}

#[derive(Debug, Clone)]
pub struct Member {
	pub mtype: ObjType,
	pub mref: i64,
	pub mrole: Arc<str>
}

#[derive(Debug, Clone)]
pub enum ObjType {
	Node, Way, Relation
}

impl<'a> From<ObjType> for &'a str {
	fn from(val: ObjType) -> &'a str {
		match val {
			ObjType::Node => "node",
			ObjType::Way => "way",
			ObjType::Relation => "relation"
		}
	}
}

impl TryFrom<Arc<str>> for ObjType {
	type Error = ReadError;
	fn try_from(val: Arc<str>) -> Result<ObjType, ReadError> {
		match &(*val) {
			"node" => Ok(ObjType::Node),
			"way" => Ok(ObjType::Way),
			"relation" => Ok(ObjType::Relation),
			_ => Err(ReadError { msg: "object type is not node/way/relation".to_string() })
		}
	}
}
