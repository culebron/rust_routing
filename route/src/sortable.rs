use std::cmp::Ord;
use core::cmp::Ordering;

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct Sortable<K: Ord, V> {
	key: K,
	val: V
}

impl<K: Ord, V> Sortable<K, V> {
	pub fn new(key: K, val: V) -> Self { Self { key, val } }
}

impl<K: Ord, V> PartialOrd for Sortable<K, V> {
	fn partial_cmp(&self, other: &Sortable<K, V>) -> Option<Ordering> { Some(self.cmp(other)) }
}

impl<K: Ord, V> Eq for Sortable<K, V> {}
impl<K: Ord, V> Ord for Sortable<K, V> {
	fn cmp(&self, other: &Sortable<K, V>) -> Ordering { self.key.cmp(&other.key).reverse() }
}

impl<K: Ord, V> PartialEq for Sortable<K, V> {
	fn eq(&self, other: &Sortable<K, V>) -> bool { self.key == other.key }
}
