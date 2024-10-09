extern crate alloc;
use alloc::{vec, vec::Vec};
use core::{
    borrow::Borrow,
    hash::{BuildHasher, Hash, Hasher},
    iter::Iterator,
    mem,
};

pub struct FNV1aHasher(u64);

impl FNV1aHasher {
    pub(super) const INITIAL_STATE: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0100_0000_01b3;
}

impl Default for FNV1aHasher {
    fn default() -> Self {
        Self(Self::INITIAL_STATE)
    }
}

impl Hasher for FNV1aHasher {
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 ^= u64::from(*b);
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

pub struct HashMap<K, V> {
    buckets: Vec<Vec<(K, V)>>,
    entry_cnt: usize,
}

impl<K, V> HashMap<K, V> {
    pub fn new() -> Self {
        Self {
            buckets: Vec::new(),
            entry_cnt: 0,
        }
    }
}

pub struct VacantEntry<'a, K: 'a, V: 'a> {
    key: K,
    map: &'a mut HashMap<K, V>,
    bucket_idx: usize,
}

impl<'a, K: 'a, V: 'a> VacantEntry<'a, K, V> {
    pub fn insert(self, value: V) -> &'a mut V
    where
        K: Hash + Eq,
    {
        self.map.buckets[self.bucket_idx].push((self.key, value));
        self.map.entry_cnt += 1;
        &mut self.map.buckets[self.bucket_idx].last_mut().unwrap().1
    }
}

pub struct OccupiedEntry<'a, K: 'a, V: 'a> {
    inner: &'a mut (K, V),
}

pub enum Entry<'a, K: 'a, V: 'a> {
    Occupied(OccupiedEntry<'a, K, V>),
    Vacant(VacantEntry<'a, K, V>),
}

impl<K, V> HashMap<K, V>
where
    K: Hash + Eq,
{
    pub fn insert(&mut self, key: K, val: V) -> Option<V> {
        self.resize_if_needed();

        let idx = self
            .bucket(&key)
            .expect("Bucket should exist at this point");
        let bucket = &mut self.buckets[idx];

        for (ref b_key, ref mut b_val) in bucket.iter_mut() {
            if key == *b_key {
                return Some(mem::replace(b_val, val));
            }
        }

        self.entry_cnt += 1;
        bucket.push((key, val));
        None
    }

    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.get(key).is_some()
    }

    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.bucket(key).and_then(|idx| {
            self.buckets[idx]
                .iter()
                .find(|(k, _)| k.borrow() == key)
                .map(|(_, v)| v)
        })
    }

    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.bucket(key).and_then(|idx| {
            self.buckets[idx]
                .iter_mut()
                .find(|(k, _)| k.borrow() == key)
                .map(|(_, v)| v)
        })
    }

    pub fn entry<'a>(&'a mut self, key: K) -> Entry<'a, K, V> {
        self.resize_if_needed();
        let bucket_idx = self
            .bucket(&key)
            .expect("Bucket should exist at this point");

        match self.buckets[bucket_idx].iter().position(|(k, _)| *k == key) {
            Some(idx) => Entry::Occupied(OccupiedEntry {
                inner: &mut self.buckets[bucket_idx][idx],
            }),
            None => Entry::Vacant(VacantEntry {
                map: self,
                key,
                bucket_idx,
            }),
        }
    }

    pub fn remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        if let Some(bucket_idx) = self.bucket(key) {
            let bucket = &mut self.buckets[bucket_idx];
            if let Some(idx) = bucket.iter().position(|(k, _)| k.borrow() == key) {
                self.entry_cnt -= 1;
                return Some(bucket.swap_remove(idx).1);
            }
        }
        None
    }

    fn bucket<Q>(&self, key: &Q) -> Option<usize>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        if self.buckets.is_empty() {
            return None;
        }

        let mut hasher = FNV1aHasher::default();
        key.hash(&mut hasher);

        let idx = (hasher.finish() % self.buckets.len() as u64) as usize;

        Some(idx)
    }

    fn resize_if_needed(&mut self) {
        // resize if load factor is 0.75
        if !self.is_empty() || self.entry_cnt < 3 * self.buckets.len() / 4 {
            return;
        }

        let size = match self.buckets.len() {
            0 => 1,
            n => 2 * n,
        };

        let mut new_buckets = Vec::with_capacity(size);
        new_buckets.extend((0..size).map(|_| Vec::new()));

        for (key, value) in self.buckets.iter_mut().flat_map(|bucket| bucket.drain(..)) {
            let mut hasher = FNV1aHasher::default();
            key.hash(&mut hasher);

            let idx = (hasher.finish() % new_buckets.len() as u64) as usize;
            new_buckets[idx].push((key, value));
        }

        let _ = mem::replace(&mut self.buckets, new_buckets);
    }

    pub fn is_empty(&self) -> bool {
        self.entry_cnt == 0
    }

    pub fn len(&self) -> usize {
        self.entry_cnt
    }

    pub fn iter<'a>(&'a self) -> Iter<'a, K, V> {
        Iter {
            map: self,
            bucket_idx: 0,
            idx: 0,
        }
    }
}

pub struct Iter<'a, K: 'a, V: 'a> {
    map: &'a HashMap<K, V>,
    bucket_idx: usize,
    idx: usize,
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(bucket) = self.map.buckets.get(self.bucket_idx) {
            match bucket.get(self.idx) {
                Some((k, v)) => {
                    self.idx += 1;
                    return Some((k, v));
                }
                None => {
                    self.bucket_idx += 1;
                    self.idx = 0;
                    continue;
                }
            }
        }
        None
    }
}
pub struct IterMut<'a, K: 'a, V: 'a> {
    map: &'a mut HashMap<K, V>,
    bucket_idx: usize,
    idx: usize,
}

impl<'a, K, V> Iterator for IterMut<'a, K, V> {
    type Item = (&'a K, &'a mut V);

    fn next(&mut self) -> Option<Self::Item> {
        while self.bucket_idx < self.map.buckets.len() {
            // SAFETY: This is safe because we ensure that we are borrowing the
            // same bucket only once per loop iteration.
            let bucket: &mut Vec<(K, V)> =
                unsafe { &mut *(&mut self.map.buckets[self.bucket_idx] as *mut _) };
            if self.idx < bucket.len() {
                let (k, v) = bucket.get_mut(self.idx)?;
                self.idx += 1;
                return Some((k, v));
            } else {
                self.bucket_idx += 1;
                self.idx = 0;
            }
        }
        None
    }
}

impl<'a, K, V> IntoIterator for &'a HashMap<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;
    fn into_iter(self) -> Self::IntoIter {
        Iter {
            map: self,
            bucket_idx: 0,
            idx: 0,
        }
    }
}

pub struct IntoIter<K, V> {
    map: HashMap<K, V>,
    bucket_idx: usize,
}

impl<K, V> Iterator for IntoIter<K, V> {
    type Item = (K, V);
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.map.buckets.get_mut(self.bucket_idx) {
                Some(bucket) => match bucket.pop() {
                    Some(x) => break Some(x),
                    None => {
                        self.bucket_idx += 1;
                        continue;
                    }
                },
                None => break None,
            }
        }
    }
}

impl<K, V> IntoIterator for HashMap<K, V> {
    type Item = (K, V);
    type IntoIter = IntoIter<K, V>;
    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            map: self,
            bucket_idx: 0,
        }
    }
}

#[cfg(test)]
mod hash_tests {
    use super::*;

    fn fnv1a(bytes: &[u8]) -> u64 {
        let mut hasher = FNV1aHasher::default();
        hasher.write(bytes);
        hasher.finish()
    }
    #[test]
    fn test() {
        assert_eq!(fnv1a(b""), 0xcbf29ce484222325);
        assert_eq!(fnv1a(b"a"), 0xaf63dc4c8601ec8c);
        assert_eq!(fnv1a(b"b"), 0xaf63df4c8601f1a5);
        assert_eq!(fnv1a(b"c"), 0xaf63de4c8601eff2);
        assert_eq!(fnv1a(b"d"), 0xaf63d94c8601e773);
        assert_eq!(fnv1a(b"e"), 0xaf63d84c8601e5c0);
        assert_eq!(fnv1a(b"f"), 0xaf63db4c8601ead9);
        assert_eq!(fnv1a(b"fo"), 0x08985907b541d342);
        assert_eq!(fnv1a(b"foo"), 0xdcb27518fed9d577);
        assert_eq!(fnv1a(b"foob"), 0xdd120e790c2512af);
        assert_eq!(fnv1a(b"fooba"), 0xcac165afa2fef40a);
        assert_eq!(fnv1a(b"foobar"), 0x85944171f73967e8);
        assert_eq!(fnv1a(b"\0"), 0xaf63bd4c8601b7df);
        assert_eq!(fnv1a(b"\0"), 0xaf63bd4c8601b7df);
        assert_eq!(fnv1a(b"a\0"), 0x089be207b544f1e4);
        assert_eq!(fnv1a(b"b\0"), 0x08a61407b54d9b5f);
        assert_eq!(fnv1a(b"c\0"), 0x08a2ae07b54ab836);
        assert_eq!(fnv1a(b"d\0"), 0x0891b007b53c4869);
        assert_eq!(fnv1a(b"e\0"), 0x088e4a07b5396540);
        assert_eq!(fnv1a(b"f\0"), 0x08987c07b5420ebb);
        assert_eq!(fnv1a(b"fo\0"), 0xdcb28a18fed9f926);
        assert_eq!(fnv1a(b"foo\0"), 0xdd1270790c25b935);
        assert_eq!(fnv1a(b"foob\0"), 0xcac146afa2febf5d);
        assert_eq!(fnv1a(b"fooba\0"), 0x8593d371f738acfe);
        assert_eq!(fnv1a(b"foobar\0"), 0x34531ca7168b8f38);
        assert_eq!(fnv1a(b"ch"), 0x08a25607b54a22ae);
        assert_eq!(fnv1a(b"cho"), 0xf5faf0190cf90df3);
        assert_eq!(fnv1a(b"chon"), 0xf27397910b3221c7);
        assert_eq!(fnv1a(b"chong"), 0x2c8c2b76062f22e0);
        assert_eq!(fnv1a(b"chongo"), 0xe150688c8217b8fd);
        assert_eq!(fnv1a(b"chongo "), 0xf35a83c10e4f1f87);
        assert_eq!(fnv1a(b"chongo w"), 0xd1edd10b507344d0);
        assert_eq!(fnv1a(b"chongo wa"), 0x2a5ee739b3ddb8c3);
        assert_eq!(fnv1a(b"chongo was"), 0xdcfb970ca1c0d310);
        assert_eq!(fnv1a(b"chongo was "), 0x4054da76daa6da90);
        assert_eq!(fnv1a(b"chongo was h"), 0xf70a2ff589861368);
        assert_eq!(fnv1a(b"chongo was he"), 0x4c628b38aed25f17);
        assert_eq!(fnv1a(b"chongo was her"), 0x9dd1f6510f78189f);
        assert_eq!(fnv1a(b"chongo was here"), 0xa3de85bd491270ce);
        assert_eq!(fnv1a(b"chongo was here!"), 0x858e2fa32a55e61d);
        assert_eq!(fnv1a(b"chongo was here!\n"), 0x46810940eff5f915);
        assert_eq!(fnv1a(b"\xff\x00\x00\x03"), 0x6961176491cc64c7);
        assert_eq!(fnv1a(b"\x03\x00\x00\xff"), 0xed205d87f40434c7);
        assert_eq!(fnv1a(b"\xff\x00\x00\x04"), 0x6961146491cc5fae);
        assert_eq!(fnv1a(b"\x04\x00\x00\xff"), 0xcd3baf5e44f8ad9c);
        assert_eq!(fnv1a(b"\x40\x51\x4e\x44"), 0xe3b36596127cd6d8);
        assert_eq!(fnv1a(b"\x44\x4e\x51\x40"), 0xf77f1072c8e8a646);
        assert_eq!(fnv1a(b"\x40\x51\x4e\x4a"), 0xe3b36396127cd372);
        assert_eq!(fnv1a(b"\x4a\x4e\x51\x40"), 0x6067dce9932ad458);
        assert_eq!(fnv1a(b"\x40\x51\x4e\x54"), 0xe3b37596127cf208);
        assert_eq!(fnv1a(b"\x54\x4e\x51\x40"), 0x4b7b10fa9fe83936);
        assert_eq!(fnv1a(b"127.0.0.1"), 0xaabafe7104d914be);
        assert_eq!(fnv1a(b"127.0.0.1\0"), 0xf4d3180b3cde3eda);
        assert_eq!(fnv1a(b"127.0.0.2"), 0xaabafd7104d9130b);
        assert_eq!(fnv1a(b"127.0.0.2\0"), 0xf4cfb20b3cdb5bb1);
        assert_eq!(fnv1a(b"127.0.0.3"), 0xaabafc7104d91158);
        assert_eq!(fnv1a(b"127.0.0.3\0"), 0xf4cc4c0b3cd87888);
        assert_eq!(fnv1a(b"64.81.78.68"), 0xe729bac5d2a8d3a7);
        assert_eq!(fnv1a(b"64.81.78.68\0"), 0x74bc0524f4dfa4c5);
        assert_eq!(fnv1a(b"64.81.78.74"), 0xe72630c5d2a5b352);
        assert_eq!(fnv1a(b"64.81.78.74\0"), 0x6b983224ef8fb456);
        assert_eq!(fnv1a(b"64.81.78.84"), 0xe73042c5d2ae266d);
        assert_eq!(fnv1a(b"64.81.78.84\0"), 0x8527e324fdeb4b37);
        assert_eq!(fnv1a(b"feedface"), 0x0a83c86fee952abc);
        assert_eq!(fnv1a(b"feedface\0"), 0x7318523267779d74);
        assert_eq!(fnv1a(b"feedfacedaffdeed"), 0x3e66d3d56b8caca1);
        assert_eq!(fnv1a(b"feedfacedaffdeed\0"), 0x956694a5c0095593);
        assert_eq!(fnv1a(b"feedfacedeadbeef"), 0xcac54572bb1a6fc8);
        assert_eq!(fnv1a(b"feedfacedeadbeef\0"), 0xa7a4c9f3edebf0d8);
        assert_eq!(fnv1a(b"line 1\nline 2\nline 3"), 0x7829851fac17b143);
    }
}

#[cfg(test)]
mod hashmap_tests {
    use super::*;

    #[test]
    fn insert() {
        let mut map = HashMap::new();
        assert_eq!(map.len(), 0);
        assert!(map.is_empty());
        map.insert("foo", 42);
        assert_eq!(map.len(), 1);
        assert!(!map.is_empty());
        assert_eq!(map.get(&"foo"), Some(&42));
        assert_eq!(map.remove(&"foo"), Some(42));
        assert_eq!(map.len(), 0);
        assert!(map.is_empty());
        assert_eq!(map.get(&"foo"), None);
    }

    #[test]
    fn iter() {
        let mut map = HashMap::new();
        map.insert("foo", 42);
        map.insert("bar", 43);
        map.insert("baz", 142);
        map.insert("quox", 7);
        for (&k, &v) in &map {
            match k {
                "foo" => assert_eq!(v, 42),
                "bar" => assert_eq!(v, 43),
                "baz" => assert_eq!(v, 142),
                "quox" => assert_eq!(v, 7),
                _ => unreachable!(),
            }
        }
        assert_eq!((&map).into_iter().count(), 4);

        let mut items = 0;
        for (k, v) in map {
            match k {
                "foo" => assert_eq!(v, 42),
                "bar" => assert_eq!(v, 43),
                "baz" => assert_eq!(v, 142),
                "quox" => assert_eq!(v, 7),
                _ => unreachable!(),
            }
            items += 1;
        }
        assert_eq!(items, 4);
    }

    #[test]
    fn empty_hashmap() {
        let mut map = HashMap::<&str, &str>::new();
        assert_eq!(map.contains_key("key"), false);
        assert_eq!(map.get("key"), None);
        assert_eq!(map.remove("key"), None);
    }

    #[test]
    fn get_mut() {
        let mut map = HashMap::new();
        map.insert(3, "foo");

        let val = map.get_mut(&3).unwrap();
        *val = "bar";

        assert_eq!(map.get(&3), Some(&"bar"));
    }

    #[test]
    fn key_collision() {
        let mut map = HashMap::new();
        map.insert(3, "foo");
        let val = map.insert(3, "bar");
        assert_eq!(val, Some("foo"))
    }

    #[test]
    fn remove() {
        let mut map = HashMap::new();
        assert_eq!(map.is_empty(), true);

        map.insert("a", 1);
        map.insert("b", 2);
        map.insert("c", 3);
        map.insert("d", 4);

        assert_eq!(map.len(), 4);
        assert_eq!(map.remove("b"), Some(2));
        assert_eq!(map.remove("d"), Some(4));
        assert_eq!(map.len(), 2);
        assert_eq!(map.remove("d"), None);
        assert_eq!(map.len(), 2);
    }
}
