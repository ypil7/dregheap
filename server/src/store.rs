use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::errors::{Error, Result};

pub type AsyncStore = Arc<Mutex<Box<dyn Store>>>;
pub const CACHE_ENTRY_OVERHEAD: usize = 64;

type NodeId = usize;

pub trait Store: Send + Sync {
    fn get(&mut self, key: &str) -> Result<Vec<u8>>;
    fn set(&mut self, key: String, val: Option<Vec<u8>>) -> Result<()>;
}

pub fn make_store() -> Box<dyn Store> {
    make_store_with_max_cache_weight(None)
}

pub fn make_store_with_max_cache_weight(max_cache_weight: Option<usize>) -> Box<dyn Store> {
    Box::new(DregStore::new(max_cache_weight))
}

pub struct DregStore {
    entries: HashMap<Arc<str>, NodeId>,
    nodes: Vec<Option<CacheEntry>>,
    free_nodes: Vec<NodeId>,
    head: Option<NodeId>,
    tail: Option<NodeId>,
    current_weight: usize,
    max_cache_weight: Option<usize>,
}

struct CacheEntry {
    key: Arc<str>,
    value: Vec<u8>,
    weight: usize,
    prev: Option<NodeId>,
    next: Option<NodeId>,
}

impl DregStore {
    pub fn new(max_cache_weight: Option<usize>) -> Self {
        Self {
            entries: HashMap::new(),
            nodes: Vec::new(),
            free_nodes: Vec::new(),
            head: None,
            tail: None,
            current_weight: 0,
            max_cache_weight,
        }
    }

    fn entry_weight(key: &str, value: &[u8]) -> Result<usize> {
        key.len()
            .checked_add(value.len())
            .and_then(|weight| weight.checked_add(CACHE_ENTRY_OVERHEAD))
            .ok_or_else(|| Error::Custom("Cache entry weight overflowed usize".to_string()))
    }

    fn reject_if_entry_too_large(&self, incoming_weight: usize) -> Result<()> {
        let Some(max_cache_weight) = self.max_cache_weight else {
            return Ok(());
        };

        if incoming_weight > max_cache_weight {
            return Err(Error::CacheEntryTooLarge(format!(
                "Cache entry weight {} exceeds maximum cache weight {}",
                incoming_weight, max_cache_weight
            )));
        }

        Ok(())
    }

    fn reject_if_unbounded_total_weight_untrackable(
        &self,
        incoming_weight: usize,
        replacing_node_id: Option<NodeId>,
    ) -> Result<()> {
        if self.max_cache_weight.is_some() {
            return Ok(());
        }

        let current_weight = if let Some(node_id) = replacing_node_id {
            self.current_weight
                .checked_sub(self.node(node_id).weight)
                .expect("cache weight should not underflow")
        } else {
            self.current_weight
        };

        current_weight
            .checked_add(incoming_weight)
            .map(|_| ())
            .ok_or_else(|| Error::Custom("Cache weight overflowed usize".to_string()))
    }

    fn ensure_capacity_for(&mut self, incoming_weight: usize) -> Result<()> {
        let Some(max_cache_weight) = self.max_cache_weight else {
            return Ok(());
        };

        while self
            .current_weight
            .checked_add(incoming_weight)
            .is_none_or(|weight| weight > max_cache_weight)
        {
            self.evict_lru_entry()?;
        }

        Ok(())
    }

    fn evict_lru_entry(&mut self) -> Result<()> {
        let Some(node_id) = self.tail else {
            return Err(Error::Custom(
                "Cache has no entries to evict, but capacity is still exceeded".to_string(),
            ));
        };

        let entry = self.remove_entry_by_id(node_id);
        self.entries.remove(entry.key.as_ref());
        Ok(())
    }

    fn insert_entry(&mut self, key: Arc<str>, value: Vec<u8>, weight: usize) -> Result<()> {
        let new_weight = self
            .current_weight
            .checked_add(weight)
            .ok_or_else(|| Error::Custom("Cache weight overflowed usize".to_string()))?;

        let node_id = self.allocate_node(CacheEntry {
            key: key.clone(),
            value,
            weight,
            prev: None,
            next: None,
        });

        self.insert_at_head(node_id);
        let previous = self.entries.insert(key, node_id);
        debug_assert!(previous.is_none());
        self.current_weight = new_weight;
        Ok(())
    }

    fn remove_entry_by_id(&mut self, node_id: NodeId) -> CacheEntry {
        self.detach_node(node_id);
        let entry = self.remove_node(node_id);
        self.current_weight = self
            .current_weight
            .checked_sub(entry.weight)
            .expect("cache weight should not underflow");
        entry
    }

    fn allocate_node(&mut self, entry: CacheEntry) -> NodeId {
        if let Some(node_id) = self.free_nodes.pop() {
            let slot = self
                .nodes
                .get_mut(node_id)
                .expect("free node id should point to an allocated slot");
            debug_assert!(slot.is_none());
            *slot = Some(entry);
            node_id
        } else {
            let node_id = self.nodes.len();
            self.nodes.push(Some(entry));
            node_id
        }
    }

    fn remove_node(&mut self, node_id: NodeId) -> CacheEntry {
        let slot = self
            .nodes
            .get_mut(node_id)
            .expect("node id should point to an allocated slot");
        let entry = slot.take().expect("node id should point to a live entry");
        self.free_nodes.push(node_id);
        entry
    }

    fn move_to_head(&mut self, node_id: NodeId) {
        if self.head == Some(node_id) {
            return;
        }

        self.detach_node(node_id);
        self.insert_at_head(node_id);
    }

    fn detach_node(&mut self, node_id: NodeId) {
        let (prev, next) = {
            let entry = self.node(node_id);
            (entry.prev, entry.next)
        };

        match prev {
            Some(prev_id) => self.node_mut(prev_id).next = next,
            None => self.head = next,
        }

        match next {
            Some(next_id) => self.node_mut(next_id).prev = prev,
            None => self.tail = prev,
        }

        let entry = self.node_mut(node_id);
        entry.prev = None;
        entry.next = None;
    }

    fn insert_at_head(&mut self, node_id: NodeId) {
        let old_head = self.head;

        {
            let entry = self.node_mut(node_id);
            entry.prev = None;
            entry.next = old_head;
        }

        match old_head {
            Some(old_head_id) => self.node_mut(old_head_id).prev = Some(node_id),
            None => self.tail = Some(node_id),
        }

        self.head = Some(node_id);
    }

    fn node(&self, node_id: NodeId) -> &CacheEntry {
        self.nodes
            .get(node_id)
            .and_then(Option::as_ref)
            .expect("node id should point to a live entry")
    }

    fn node_mut(&mut self, node_id: NodeId) -> &mut CacheEntry {
        self.nodes
            .get_mut(node_id)
            .and_then(Option::as_mut)
            .expect("node id should point to a live entry")
    }
}

impl Store for DregStore {
    fn get(&mut self, key: &str) -> Result<Vec<u8>> {
        let Some(&node_id) = self.entries.get(key) else {
            return Err(Error::Custom("Value not found".to_string()));
        };

        let value = self.node(node_id).value.clone();
        self.move_to_head(node_id);
        Ok(value)
    }

    fn set(&mut self, key: String, val: Option<Vec<u8>>) -> Result<()> {
        match val {
            Some(value) => {
                let weight = Self::entry_weight(&key, &value)?;
                self.reject_if_entry_too_large(weight)?;
                let existing_node_id = self.entries.get(key.as_str()).copied();
                self.reject_if_unbounded_total_weight_untrackable(weight, existing_node_id)?;

                if let Some(node_id) = existing_node_id {
                    self.entries.remove(key.as_str());
                    self.remove_entry_by_id(node_id);
                }

                self.ensure_capacity_for(weight)?;

                self.insert_entry(Arc::from(key.into_boxed_str()), value, weight)?;
            }
            None => {
                if let Some(node_id) = self.entries.remove(key.as_str()) {
                    self.remove_entry_by_id(node_id);
                }
            }
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value_weight(key: &str, value: &[u8]) -> usize {
        DregStore::entry_weight(key, value).expect("test entry weight should fit")
    }

    #[test]
    fn rejects_single_write_that_exceeds_max_cache_weight() {
        let mut store = DregStore::new(Some(value_weight("a", b"tiny")));

        let err = store
            .set("oversized".to_string(), Some(b"this will not fit".to_vec()))
            .expect_err("oversized write should be rejected");

        assert!(err.to_string().contains("exceeds maximum cache weight"));
        assert!(store.get("oversized").is_err());
    }

    #[test]
    fn evicts_least_recently_used_entry_on_write() {
        let first_weight = value_weight("first", b"1");
        let second_weight = value_weight("second", b"2");
        let third_weight = value_weight("third", b"3");
        let mut store = DregStore::new(Some(first_weight + second_weight + third_weight - 1));

        store.set("first".to_string(), Some(b"1".to_vec())).unwrap();
        store
            .set("second".to_string(), Some(b"2".to_vec()))
            .unwrap();
        store.set("third".to_string(), Some(b"3".to_vec())).unwrap();

        assert!(store.get("first").is_err());
        assert_eq!(store.get("second").unwrap(), b"2");
        assert_eq!(store.get("third").unwrap(), b"3");
    }

    #[test]
    fn get_refreshes_lru_recency() {
        let first_weight = value_weight("first", b"1");
        let second_weight = value_weight("second", b"2");
        let third_weight = value_weight("third", b"3");
        let mut store = DregStore::new(Some(first_weight + second_weight + third_weight - 1));

        store.set("first".to_string(), Some(b"1".to_vec())).unwrap();
        store
            .set("second".to_string(), Some(b"2".to_vec()))
            .unwrap();
        assert_eq!(store.get("first").unwrap(), b"1");
        store.set("third".to_string(), Some(b"3".to_vec())).unwrap();

        assert_eq!(store.get("first").unwrap(), b"1");
        assert!(store.get("second").is_err());
        assert_eq!(store.get("third").unwrap(), b"3");
    }

    #[test]
    fn delete_releases_cache_weight() {
        let first_weight = value_weight("first", b"1");
        let second_weight = value_weight("second", b"2");
        let mut store = DregStore::new(Some(first_weight + second_weight));

        store.set("first".to_string(), Some(b"1".to_vec())).unwrap();
        store
            .set("second".to_string(), Some(b"2".to_vec()))
            .unwrap();
        store.set("first".to_string(), None).unwrap();
        store.set("third".to_string(), Some(b"3".to_vec())).unwrap();

        assert!(store.get("first").is_err());
        assert_eq!(store.get("second").unwrap(), b"2");
        assert_eq!(store.get("third").unwrap(), b"3");
    }

    #[test]
    fn updating_existing_key_moves_it_to_most_recent_position() {
        let first_weight = value_weight("first", b"1");
        let second_weight = value_weight("second", b"2");
        let third_weight = value_weight("third", b"3");
        let mut store = DregStore::new(Some(first_weight + second_weight + third_weight - 1));

        store.set("first".to_string(), Some(b"1".to_vec())).unwrap();
        store
            .set("second".to_string(), Some(b"2".to_vec()))
            .unwrap();
        store
            .set("first".to_string(), Some(b"updated".to_vec()))
            .unwrap();
        store.set("third".to_string(), Some(b"3".to_vec())).unwrap();

        assert_eq!(store.get("first").unwrap(), b"updated");
        assert!(store.get("second").is_err());
        assert_eq!(store.get("third").unwrap(), b"3");
    }

    #[test]
    fn reuses_freed_node_ids() {
        let mut store = DregStore::new(None);

        store.set("first".to_string(), Some(b"1".to_vec())).unwrap();
        let first_id = *store
            .entries
            .get("first")
            .expect("first entry should have a node id");

        store.set("first".to_string(), None).unwrap();
        store
            .set("second".to_string(), Some(b"2".to_vec()))
            .unwrap();

        let second_id = *store
            .entries
            .get("second")
            .expect("second entry should have a node id");
        assert_eq!(second_id, first_id);
    }
}
