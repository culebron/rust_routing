
use osmgraph::{NodeChain, ChainStorage, find_vertice, iterate_objs, get_road};
use osmpbfreader::OsmPbfReader;
use std::error::Error;
use std::ffi::OsStr;
use std::fs::File;

#[allow(dead_code, unused_imports)]
use osmpbfreader::objects::{NodeId, WayId, OsmObj, Way};
#[allow(unused_imports)]
use std::thread;
use crossbeam_channel::{bounded, Sender, Receiver};

use std::path::Path;
use gdal::{Driver, LayerOptions};
use gdal::spatial_ref::SpatialRef;
use gdal::vector::{FieldDefn, ToGdal, FieldValue};
use gdal::errors::GdalError;
use gdal::vector::OGRFieldType;
use geo_types::{coord, LineString};

fn get_reader(input_path: &OsStr) -> Result<OsmPbfReader<File>, Box<dyn Error>> {
	let file = File::open(&Path::new(input_path))?;
	Ok(OsmPbfReader::new(file))
}


fn process_graph(osm_file: &OsStr, output_file: &OsStr) -> Result<(), Box<dyn Error>> {
	let ts: i64 = chrono::offset::Local::now().timestamp();

	let mut reader = get_reader(osm_file)?;
	let (vertice, node_coords) = find_vertice(&mut reader)?;

	let mut threads:Vec<thread::JoinHandle<ChainStorage>> = vec![];
	let (s1, r1):(Sender<Way>, Receiver<Way>) = bounded(10);
	let (s2, r2):(Sender<Vec<NodeChain>>, Receiver<Vec<NodeChain>>) = bounded(10);

	for i in 0..8 {
		let mut cs = ChainStorage::new(&vertice);
		let r1_ = r1.clone();
		let s2_ = s2.clone();
		threads.push(thread::spawn(move || {
			while let Ok(w) = r1_.recv() {
				//println!("thread {} receieved {:?}", i, w.id);
				s2_.send(cs.insert_way(w)).unwrap();
			}
			drop(s2_);
			println!("thread {:?} ending", i);
			cs
		}));
	};
	let ts2: i64 = chrono::offset::Local::now().timestamp();
	println!("{:?} seconds, buliding graph", &ts2 - &ts);
	let ppp = Path::new(&output_file);
	let drv = Driver::get("GPKG")?;
	let mut dataset = drv.create_vector_only(&ppp)?;
	//let mut dataset = Dataset::open_ex(&ppp, dso).expect("failed to open gpkg file");

	let writer_thread = thread::spawn(move || -> Result<(), GdalError> {
		//let mut layer = dataset.layer(0).unwrap();
		//let mut area:f64 = 0.0;
		//let mut rows:i32 = 0;
		let wgs = SpatialRef::from_epsg(4326).unwrap();
		let opts = LayerOptions {
			name: "edges",
			srs: Some(&wgs),
			..Default::default()
		};
		let mut lyr = dataset.create_layer(opts)?;

		let field_defn = FieldDefn::new("node_start", OGRFieldType::OFTInteger64)?;
		field_defn.add_to_layer(&lyr)?;

		let field_defn = FieldDefn::new("node_end", OGRFieldType::OFTInteger64)?;
		field_defn.add_to_layer(&lyr)?;

		let field_defn = FieldDefn::new("category", OGRFieldType::OFTString)?;
		field_defn.set_width(20);
		field_defn.add_to_layer(&lyr)?;

		//let defn = Defn::from_layer(&lyr);

		let fields = ["node_start", "node_end", "category"];
		while let Ok(edges) = r2.recv() {

			for edge in edges.into_iter() {
				/*let linestring = format!("LINESTRING({})", edge.nodes.iter().filter_map(|nid| node_coords.get(&nid)).map(|y| format!("{} {}", y[0], y[1])).collect::<Vec<String>>().join(", "));
				// println!("{:?}, {}", edge.nodes, linestring);
				let ends = edge.ends();
				let ce = CsvEdge { WKT: linestring, node1: ends[0], node2: ends[1], category: edge.category.to_string(), lanes: edge.lanes };
				writer.serialize(ce).unwrap();*/

				let ls: LineString<f32> = edge.nodes.iter()
					.filter_map(|nid| node_coords.get(&nid))
					.map(|cc| coord! {x: cc[0], y: cc[1]}).collect();
				let ends = edge.ends();

				lyr.create_feature_fields(ls.to_gdal()?,
					&fields,
					&[
						FieldValue::Integer64Value(ends[0].0),
						FieldValue::Integer64Value(ends[1].0),
						FieldValue::StringValue(edge.category.to_string())
					])?;
			}
		}
		Ok(())
	});

	for w in iterate_objs(&mut reader, &mut get_road)? {
		s1.send(w).unwrap();
	}
	drop(s1);

	let mut result_cs = ChainStorage::new(&vertice);
	for th in threads {
		let mut cs2 = th.join().unwrap();
		result_cs.vertice.extend(cs2.vertice.clone());
		for (_, e) in cs2.edges.clone().into_iter() {
			cs2.remove(&e);
			s2.send(result_cs.insert(e)).unwrap();
		}
	}
	drop(s2);
	writer_thread.join();
	println!("graph built in {:?} seconds", chrono::offset::Local::now().timestamp() - &ts2);

	Ok(())
}


fn main() {
	let ts: i64 = chrono::offset::Local::now().timestamp();
	let args: Vec<_> = std::env::args_os().collect();
	//let ts: i64 = chrono::offset::Local::now().timestamp();

	match args.len() {
		3 => {
			let _ = process_graph(&args[1], &args[2]);
		}
		_ => println!("usage: osmgraph INPUT.OSM.PBF OUTPUT_FILE",),
	};
	println!("{:?} seconds", chrono::offset::Local::now().timestamp() - &ts);
}

