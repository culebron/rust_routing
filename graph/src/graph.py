from shapely.geometry import LineString, Point
import osmium as osm
from erde import autocli, IPDB, write_df, write_stream
import geopandas as gpd
from tqdm import tqdm
import numpy as np


def deb_print(*args, **kwargs):
	if IPDB:
		print(*args, **kwargs)


class NodeCounter(osm.SimpleHandler):
	def __init__(self, tqdm_=None):
		super().__init__()  # без этого segfault
		self.node_counters = []
		self.t = tqdm_

	def way(self, w):
		if 'highway' not in w.tags:
			return

		nrefs = [n.ref for n in w.nodes]
		self.node_counters.append(np.array(nrefs + nrefs[1:-1], 'int64'))
		self.t.update()

	@property
	def vertice_nodes(self):
		arrr = np.concatenate(self.node_counters)
		del self.node_counters[:]
		unique, counts = np.unique(arrr, return_counts=True)
		self.good_counts = dict(zip(unique[counts != 2], counts[counts != 2]))
		return set(np.extract(counts != 2, unique))


class NodeChain:
	def __init__(self, refs, coords, osm_id, attrs):
		self.refs = refs
		self.coords = coords
		self.osm_id = osm_id  # если цепочка склеена из двух разных с разным osm_id, это поле будет пустым
		self.attrs = attrs

	@property
	def start(self):
		return self.refs[0]

	@property
	def end(self):
		return self.refs[-1]

	@property
	def endings(self):
		return self.refs[0], self.refs[-1]

	def __len__(self):
		return len(self.refs)

	def __getitem__(self, key):
		if isinstance(key, int):
			return self.refs[key]  # хитрость: если попросить по одиночному индексу, выдаст id узла!

		return NodeChain(self.refs[key], self.coords[key], self.osm_id, self.attrs)  # слайсы выдают цепочку, но в ней узлы отслайсены

	def __add__(self, other):
		if not self.can_add(other):  # чтобы при программировании в явном виде проверять, можно ли склеить, иначе будет нечитаемо
			raise ValueError

		if self.end == other.start:
			return NodeChain(self.refs + other.refs[1:], self.coords + other.coords[1:], None, self.attrs)

		if self.end == other.end:
			return self + other[::-1]

		if self.start == other.start:
			return self[::-1] + other

		if self.start == other.end:
			return other + self

		raise ValueError("Can't concat these chains, no common nodes.")

	def can_add(self, other):
		return self.attrs == other.attrs

	def common_nodes(self, other):
		if self.start in other.endings:
			yield self.start
		if self.end in other.endings:
			yield self.end

	def __repr__(self):
		inter = f'-({len(self) - 2})-' if len(self) > 2 else '-'
		return f'<NodeChain {self.refs[0]:,}{inter}{self.refs[-1]:,}>'


class WayProcessor(osm.SimpleHandler):
	DEFAULT_ATTRS = ('highway', 'lanes', 'maxspeed', 'maxspeed_practical', 'oneway')
	def __init__(self, vertice_nodes, write_callback, total=None, keep_attributes=DEFAULT_ATTRS, tqdm_=None):
		super().__init__()  # иначе segfault
		self.t = tqdm_
		self.vertice_nodes = vertice_nodes
		self.vertice_coords = {}
		self.original_vertice_coords = {}
		self.merge_nodes = {}  # mapping: node => way
		self._callback = write_callback
		self.edge_counter = 0
		self.check = True
		self.attrs = {}  # nodes => attributes dicts
		self.keep_attributes = keep_attributes

	def write_callback(self, chain):
		deb_print(f'WRITE {chain}')
		self.edge_counter += 1
		self._callback(chain, self.edge_counter)

	def concat(self, left_chain):
		for n in left_chain.endings:
			if n in self.merge_nodes and self.merge_nodes[n] != left_chain:
				break
		else:
			return left_chain

		right_chain = self.merge_nodes[n]
		for c in left_chain, right_chain:
			for n in c.endings:
				if self.merge_nodes.get(n, None) == c:
					self.merge_nodes.pop(n, None)

		if left_chain.can_add(right_chain):
			return self.concat(left_chain + right_chain)

		deb_print('can\'t merge because different tags')
		for n in left_chain.common_nodes(right_chain):
			self.vertice_nodes.add(n)
			self.merge_nodes.pop(n, None)

		for c in (left_chain, right_chain):
			for i in (0, -1):
				self.vertice_coords[c.refs[i]] = c.coords[i]

		self.merge_chains(right_chain)
		self.merge_chains(left_chain)

	def merge_chains(self, chain):
		deb_print(f'merging {chain}')
		if all(n in self.vertice_nodes for n in chain.endings):
			deb_print('ends are vertices, no merge')
			self.write_callback(chain)
			return

		while True:
			new_chain = self.concat(chain)
			deb_print('done concat')
			if new_chain is None:
				deb_print('no return from concat')
				return

			for n in new_chain.endings:
				if n not in self.vertice_nodes:
					self.merge_nodes[n] = new_chain

			if all(n in self.vertice_nodes for n in chain.endings):
				self.write_callback(chain)
				return

			if new_chain == chain:
				return

			chain = new_chain

	def way(self, w):
		if 'highway' not in w.tags:
			return

		self.t.update()
		try:
			refs = tuple(n.ref for n in w.nodes)
			coords = tuple((n.lon, n.lat) for n in w.nodes)
		except osm._osmium.InvalidLocationError:
			return

		for n in w.nodes:
			if n.ref in self.vertice_nodes:
				self.vertice_coords[n.ref] = (n.lon, n.lat)
				self.original_vertice_coords[n.ref] = (n.lon, n.lat)

		attrs = {k: w.tags.get(k) for k in self.keep_attributes}
		chain = NodeChain(refs, coords, w.id, attrs)

		deb_print(f'way {w.id:,}, length {len(chain)}')

		if len(chain) == 2:
			self.merge_chains(chain)
			return

		prev_vertice = 0
		for i, ref in enumerate(chain.refs):
			if ref in self.vertice_nodes and i > prev_vertice:
				deb_print(f'vertice at {i}, {ref:,}')
				self.merge_chains(chain[prev_vertice:i + 1])
				prev_vertice = i

		if prev_vertice < len(chain) - 1:
			deb_print('tail without vertice, trying to merge')
			self.merge_chains(chain[prev_vertice:])


def count_ways(path):
	with tqdm(desc='Counting ways') as t:
		class WayCounter(osm.SimpleHandler):
			def way(self, w):
				if 'highway' not in w.tags:
					return
				t.update()

		wc = WayCounter()
		wc.apply_file(path)
		return t.n


@autocli
def main(input_file, output_path, vertices_path=None, keep_columns=None):
	if keep_columns is None:
		keep_columns = WayProcessor.DEFAULT_ATTRS
	elif keep_columns == '':
		keep_columns = []
	else:
		keep_columns = keep_columns.split(',')
	
	ways_total = count_ways(input_file)
	with tqdm(desc='Counting ways\' nodes', total=ways_total) as t1:
		nc = NodeCounter(t1)
		nc.apply_file(input_file)

	with write_stream(output_path) as write_chunk, tqdm(desc='Processing ways', total=ways_total) as t2:
		ds = []

		def write_data():
			df = gpd.GeoDataFrame(ds, columns=['osm_id', 'edge_id', 'start_node', 'end_node', 'geometry', *keep_columns], crs=4326)
			del ds[:]
			write_chunk(df)

		def fn(chain, edge_counter):
			if len(chain) < 2:
				return

			if len(chain) == 2:
				if chain.coords[0] == chain.coords[1]:
					return

			ds.append([chain.osm_id, edge_counter, chain.start, chain.end, LineString(chain.coords), *[chain.attrs[k] for k in keep_columns]])
			if len(ds) > 10000:
				write_data()

		wp = WayProcessor(nc.vertice_nodes, fn, nc.t.n, tqdm_=t2)

		if vertices_path is None:
			del nc.node_counters[:]
			del nc
		deb_print('\n\n\n')

		wp.apply_file(input_file, locations=True)  # locations=True чтобы в линии в nodes были сразу координаты
		write_data()

	if vertices_path is not None:
		print(f'saving {len(wp.vertice_coords)} vertices to {vertices_path}')
		gdf = gpd.GeoDataFrame([(k, Point(*v), nc.good_counts[k]) for k, v in wp.original_vertice_coords.items()], columns=['node_id', 'geometry', 'count'], crs=4326)
		write_df(gdf, vertices_path)
