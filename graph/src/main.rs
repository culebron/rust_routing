use osmio2::reader::OsmXmlReader;
use geo::LineString;
use csv::Writer;
use osmgraph::{ChainStorage, find_vertice, Edge};

use std::{
	error::Error,
	path::Path,
	time::SystemTime,
};

fn process_graph(osm_file: &str, output_file: &str) -> Result<(), Box<dyn Error>> {
	let t1 = SystemTime::now();

	let (vertice, node_coords) = find_vertice(&osm_file)?;

	let t2 = SystemTime::now();
	println!("{} s, building graph", t2.duration_since(t1)?.as_secs_f32());

	let ppp = Path::new(&output_file);
	let mut writer = Writer::from_path(ppp)?;

	let mut cs = ChainStorage::new(&vertice);

	let mut rd = OsmXmlReader::from_path(&osm_file)?;

	rd.map_ways(|w| {
		for edge in cs.insert_way(w).into_iter() {
			let ee = edge.nodes.iter().filter_map(|nid| node_coords.get(&nid)).map(|t| t.clone()).collect::<Vec<(f64, f64)>>();

			let linestring = LineString::from(ee);
			let ends = edge.ends();
			writer.serialize(Edge {
				WKT: linestring,
				node1: ends[0],
				node2: ends[1],
				category: edge.category,
				lanes: edge.lanes,
				oneway: edge.oneway,
				maxspeed: edge.maxspeed,
			})?;
		}
		Ok(())
	})?;

	let t3 = SystemTime::now();
	println!("{} s, building graph", t3.duration_since(t2)?.as_secs_f32());

	Ok(())
}


fn main() -> Result<(), Box<dyn Error>> {
	let args: Vec<_> = std::env::args_os().collect();
	match args.len() {
		3 => {
			process_graph(&args[1].to_str().unwrap(), &args[2].to_str().unwrap())?;
		}
		_ => println!("usage: osmgraph INPUT.OSM.PBF OUTPUT_FILE",),
	};
	Ok(())
}

