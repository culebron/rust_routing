use crate::{
	objects::RawEdge
};
use csv::Reader;
use serde::Deserialize;
use geo::LineString;
use std::{
	error::Error,
	collections::{BTreeSet, HashMap},
};

pub struct Vid(usize);
pub struct EdgeOffset(usize);
pub struct Weight(u16);

pub struct Edg {
	pub from: Vid, // number of vertex in vertice vec (Vec<EdgOffset>)
	pub to: Vid, // same
	pub geom: LineString,
	pub category: String,
	pub lanes: u8
}


pub fn create_hedgehog(path: &str) -> Result<(), Box<dyn Error>> {
	let mut vids: BTreeSet<i64> = BTreeSet::new();
	let mut rd = Reader::from_path(path)?;

	for rec in rd.deserialize() {
		// составление списка id вершин
		let RawEdge { node1, node2, .. } = rec?;
		vids.insert(node1.0);
		vids.insert(node2.0);
	}
	let vids = HashMap::<i64, usize>::from(vids.iter().collect::<Vec<i64>>());

	let vertice: Vec<EdgeOffset> = vec![];
	let orig_edges: Vec<Edg> = vec![];
	let graph: Vec<Edg> = vec![];

	Ok(())
}
