use serde::{Serialize, Deserialize};
use geo::{LineString, Point};
use crate::serialize_wkt;
use std::{
	cmp::Ordering, ops::{Add, Sub}, collections::HashMap,
	time::SystemTime, error::Error,
};


pub type VisitedMap = HashMap<VertexId, VertexScore>;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct VertexId(pub i64);

#[derive(Clone, Copy, Debug)]
pub struct Weight(pub u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Cost(pub i64);

impl Add<Cost> for Cost {
	type Output = Cost;
	fn add(self, other: Cost) -> Cost { Cost(self.0 + other.0) }
}

impl Sub<Cost> for Cost {
	type Output = Cost;
	fn sub(self, other: Cost) -> Cost { Cost(self.0.checked_sub(other.0).unwrap_or(0)) }
}

impl Add<Weight> for Cost {
	type Output = Cost;
	fn add(self, other: Weight) -> Cost { Cost(self.0 + other.0 as i64) }
}

#[derive(Clone, Debug)]
pub struct Vertex {
	pub id: VertexId,
	pub geom: Point,
	pub edges: Vec<Edge>,
}

#[derive(Clone, Debug)]
pub struct Edge {
	pub v1: VertexId,
	pub v2: VertexId,
	pub weight: Weight,
	pub geom: LineString,
}

#[derive(Clone, Debug)]
pub struct VertexScore {
	pub vid: VertexId,
	pub from: VertexId,
	pub cost_before: Cost,
	pub cost_remain: Cost,
	pub total_cost: Cost,
	pub visit_number: isize,
}

impl VertexScore {
	pub fn new(vid: VertexId, from: VertexId, cost_before: Cost, cost_remain: Cost) -> Self {
		Self { vid, from, cost_before, cost_remain, total_cost: cost_before + cost_remain, visit_number: -1 }
	}
}
impl PartialOrd for VertexScore {
	fn partial_cmp(&self, other: &VertexScore) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

impl Eq for VertexScore {}
impl Ord for VertexScore {
	fn cmp(&self, other: &VertexScore) -> Ordering {
		self.total_cost.cmp(&other.total_cost).reverse()
	}
}

impl PartialEq for VertexScore {
	fn eq(&self, other: &VertexScore) -> bool {
		self.total_cost == other.total_cost
	}
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RawEdge {
	pub node1: VertexId,
	pub node2: VertexId,
	#[serde(with = "serialize_wkt")]
	pub WKT: LineString,
	pub category: String,
	pub lanes: u8
}


pub struct TimeCheck(SystemTime);
impl TimeCheck {
	pub fn new() -> Self { TimeCheck(SystemTime::now()) }
	pub fn delta(&mut self) -> Result<f32, Box<dyn Error>> {
		let old_time = self.0;
		self.0 = SystemTime::now();
		Ok(self.0.duration_since(old_time)?.as_secs_f32())
	}
}
