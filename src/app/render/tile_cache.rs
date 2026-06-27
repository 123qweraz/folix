use std::collections::HashMap;

pub struct TileCache {
    cache: HashMap<usize, Vec<u8>>,
    max_tiles: usize,
}

impl TileCache {
    pub fn new(max_tiles: usize) -> Self {
        Self {
            cache: HashMap::new(),
            max_tiles,
        }
    }

    pub fn get(&self, key: &usize) -> Option<&Vec<u8>> {
        self.cache.get(key)
    }

    pub fn insert(&mut self, key: usize, data: Vec<u8>) {
        if self.cache.len() >= self.max_tiles {
            if let Some(oldest) = self.cache.keys().next().cloned() {
                self.cache.remove(&oldest);
            }
        }
        self.cache.insert(key, data);
    }
}
