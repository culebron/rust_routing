use geo::LineString;
use serde::{Serialize, Deserialize};

use osmio2::serialize_wkt;
use osmio2::{
	reader::OsmXmlReader,
	objects::{Way, NodeId, WayId}
};

use std::{collections::{HashSet, HashMap},
	sync::Arc, error::Error, fmt,
};

#[allow(dead_code, unused_imports)]
pub type WaysInNodesCounter = HashMap<NodeId, i32>;

pub type NodeCoords = HashMap<NodeId, (f64, f64)>;

#[allow(non_snake_case)]
#[derive(Serialize)]
pub struct Edge {
	pub node1: NodeId,
	pub node2: NodeId,
	#[serde(with = "serialize_wkt")]
	pub WKT: LineString,
	pub category: RoadCat,
	pub lanes: u8,
	pub oneway: OneWay,
	pub maxspeed: MaxSpeed,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum RoadCat {
	Motorway,
	Trunk,
	Primary,
	Secondary,
	Tertiary,
	Residential,
	Living,
	Unclassified,
	Road,
	Service,
	Cycleway,
	Footway,
	Pedestrian,
	Steps,
	Path,
	Track,
	Bridleway
}


fn parse_road_cat(s: Option<&Arc<str>>) -> Option<RoadCat> {
	if let Some(s2) = s {
		match &**s2 {
			"bridleway"      => Some(RoadCat::Bridleway),
			"cycleway"       => Some(RoadCat::Cycleway),
			"footway"        => Some(RoadCat::Footway),
			"living_street"  => Some(RoadCat::Living),
			"motorway"       => Some(RoadCat::Motorway),
			"path"           => Some(RoadCat::Path),
			"pedestrian"     => Some(RoadCat::Pedestrian),
			"primary"        => Some(RoadCat::Primary),
			"primary_link"   => Some(RoadCat::Primary),
			"residential"    => Some(RoadCat::Residential),
			"road"           => Some(RoadCat::Road),
			"secondary"      => Some(RoadCat::Secondary),
			"secondary_link" => Some(RoadCat::Secondary),
			"service"        => Some(RoadCat::Service),
			"steps"          => Some(RoadCat::Steps),
			"tertiary"       => Some(RoadCat::Tertiary),
			"tertiary_link"  => Some(RoadCat::Tertiary),
			"track"          => Some(RoadCat::Track),
			"trunk"          => Some(RoadCat::Trunk),
			"trunk_link"     => Some(RoadCat::Trunk),
			"unclassified"   => Some(RoadCat::Unclassified),
			_ => None
		}
	} else {
		None
	}
}

#[derive(Debug, Clone)]
pub struct StubError;
impl Error for StubError {}

impl fmt::Display for StubError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "Stub error")
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum OneWay { No, Forward, Backward }

impl From<Option<&Arc<str>>> for OneWay {
	fn from(data: Option<&Arc<str>>) -> Self {
		data.map(|a| {
			match &**a {
				"yes" => Self::Forward,
				"-1" => Self::Backward,
				_ => Self::No,
			}
		}).unwrap_or(Self::No)
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MaxSpeed(Option<u16>);

impl From<Option<&Arc<str>>> for MaxSpeed {
	fn from(data: Option<&Arc<str>>) -> Self {
		Self(data.map(|v| {
			match v.parse::<u16>() {
				Err(_) => None,
				Ok(w) => Some(w)
			}
		}).unwrap_or(None))
	}
}


#[derive(Clone, Debug)]
pub struct NodeChain {
	pub way_id: Option<WayId>,
	pub nodes: Vec<NodeId>,
	pub category: RoadCat,
	pub lanes: u8,
	pub oneway: OneWay,
	pub maxspeed: MaxSpeed,
}


impl NodeChain {
	pub fn new(way_id: i64, nodes: &[i64], category: RoadCat, lanes: u8, oneway: OneWay, maxspeed: MaxSpeed) -> Self {
 		Self {
			way_id: Some(WayId(way_id)),
			nodes: nodes.iter().map(|n| NodeId(*n)).collect(),
			category, lanes, oneway, maxspeed
		}
	}

	pub fn ends(&self) -> [NodeId; 2] {
		[self.nodes[0].clone(), self.nodes[self.nodes.len() - 1].clone()]
	}

	pub fn couple(&self, other: &NodeChain) -> Option<NodeChain> {
		if self.oneway != other.oneway || self.category != other.category || self.lanes != other.lanes {
			return None
		}

		let se = self.ends();
		let oe = other.ends();
		let mine = &self.nodes;
		let their = &other.nodes;

		let new_nodes:Vec<NodeId> = if se[0] == oe[0] {
			mine[1..].iter().rev().into_iter().chain(their.iter()).cloned().collect()
		} else if se[0] == oe[1] {
			their.iter().chain(mine[1..].iter()).cloned().collect()
		} else if se[1] == oe[0] {
			mine.iter().chain(their[1..].iter()).cloned().collect()
		} else if se[1] == oe[1] {
			mine[..mine.len() - 1].iter().chain(their.iter().rev()).cloned().collect()
		} else {
			return None;
		};

		Some(NodeChain {
			way_id: None,
			nodes: new_nodes,
			category: self.category.clone(),
			lanes: self.lanes,
			oneway: self.oneway,
			maxspeed: self.maxspeed,
		})
	}
}


pub type VerticeHash = HashSet<NodeId>;

pub struct ChainStorage {
	pub edges: HashMap<NodeId, NodeChain>,
	pub vertice: HashSet<NodeId>
}

impl ChainStorage {
	pub fn new(vertice: &VerticeHash) -> ChainStorage {
		ChainStorage {
			edges: HashMap::new(),
			vertice: vertice.clone()
		}
	}

	pub fn remove(&mut self, nc: &NodeChain) {
		for e in nc.ends() {
			self.edges.remove(&e);
		}
	}

	pub fn insert_way(&mut self, way: Way) -> Vec<NodeChain> {
		let end = way.nodes.len() - 1;
		let mut prev_i:usize = 0;
		let mut res: Vec<NodeChain> = vec![];
		let cat = match parse_road_cat(way.tags.get("highway")) {
			Some(c) => c,
			None => return vec![],
		};

		let ow = OneWay::from(way.tags.get("oneway"));
		let ms = MaxSpeed::from(way.tags.get("maxspeed"));

		for (cur_i, node) in way.nodes.iter().enumerate() {
			if (cur_i > prev_i && self.vertice.contains(&NodeId(*node))) || cur_i == end {
				let new_nc = NodeChain::new(way.attrs.id, &way.nodes[prev_i..cur_i + 1],
					cat.clone(), 1, ow, ms);
				res.push(new_nc);
				prev_i = cur_i;
			}
		}
		res.into_iter().map(|i| self.insert(i)).flatten().collect()
	}

	pub fn insert(&mut self, nc: NodeChain) -> Vec<NodeChain> {

		// если путь приклеился к тому, что в хранилище, но получилось всё равно не полное ребро, то он сохраняется внутри и не выдаётся ничего
		// или пустой вектор

		// то, что выдаётся -- писать в файл

		let ends = nc.ends();

		// если у пути оба конца -- вершины, он выдаётся обратно
		if self.vertice.contains(&ends[0]) && self.vertice.contains(&ends[1]) {
			return vec![nc];
		}

		// если путь не с вершиной, но приклеился к тому, что лежало внутри, пытаемся вставить получившееся снова. вставляем пока не окажется полным ребром или не будет вставлен.
		for e in ends {
			if self.vertice.contains(&e) { continue }
			match self.edges.get(&e) { // nc2 causes a borrow here
				None => {}
				Some(nc2) => {
					let nc2_ = (*nc2).clone(); // nc2 causes a borrow, should be cloned
					match nc.couple(nc2) {
						Some(nc3) => {
							self.remove(&nc2_);
							return self.insert(nc3)
						},
						None => {
							/* if 2 chains could not couple, the node between them is a vertex
							put it to vertice hashmap and try reinserting the lines
							(some of them will be returned as full edges, some will be stored) */
							self.vertice.insert(e);
							return self.insert(nc).into_iter().chain(self.insert(nc2_).into_iter()).collect()
						}
					}
				}
			}
		}

		// did not get attached to anything in self.edges, and has dangling ends
		for e in ends {
			if !self.vertice.contains(&e) {
				self.edges.insert(e, nc.clone());
			}
		}
		vec![]
	}

}


pub fn find_vertice(path: &str) -> Result<(VerticeHash, NodeCoords), Box<dyn Error>> {
	let mut rd = OsmXmlReader::from_path(&path)?;
	let mut used_nodes = WaysInNodesCounter::new();
	println!("map ways to find vertice");
	rd.map_ways(|way| {
		if parse_road_cat(way.tags.get("highway")).is_some() {
			for node_id in [&way.nodes, &way.nodes[1..way.nodes.len() - 1]].concat() {
				*used_nodes.entry(NodeId(node_id)).or_insert(0) += 1;
			}
		}
		Ok(())
	})?;
	drop(rd);
	println!("nodes: {}", used_nodes.len());

	let mut node_coords = NodeCoords::new();
	let mut rd = OsmXmlReader::from_path(&path)?;
	println!("map nodes to find vertice");
	rd.map_nodes(|n| {
		let nid = NodeId(n.attrs.id);
		if used_nodes.contains_key(&nid) {
			node_coords.insert(nid, (n.lon as f64, n.lat as f64));
		}
		Ok(())
	})?;
	drop(rd);
	println!("node coords: {}", node_coords.len());

	let vertice: VerticeHash = used_nodes.into_iter().filter(|(_, v)| *v != 2).map(|(k, _)| k).collect();
	Ok((vertice, node_coords))
}

