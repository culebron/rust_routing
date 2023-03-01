use crate::{
	objects::{Cost, Edge, VertexId, VisitedMap},
	errors::RoutingError,
	graph::Graph,
};
use geo::Point;

pub trait GraphPath {
	fn cost(&self) -> Result<Cost, RoutingError>;
	fn edges(&self) -> Result<Vec<Edge>, RoutingError>;
	fn edge_nums(&self) -> Result<Vec<(i64, i64)>, RoutingError>;
	fn vertice(&self) -> Result<Vec<VertexId>, RoutingError>;
	fn vertice_nums(&self) -> Result<Vec<i64>, RoutingError>;
	fn vertice_geoms(&self) -> Result<Vec<Point>, RoutingError>;
	fn visited(&self) -> Vec<&VisitedMap>;
}


pub trait Router {
	type ResultPath: GraphPath;
	fn shortest_path(&self, source: &VertexId, target: &VertexId) -> Result<Self::ResultPath, RoutingError>;
	fn get_graph(&self) -> &Graph;
}
