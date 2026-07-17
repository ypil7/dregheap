use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::errors::{Error, Result};

pub type AsyncStore = Arc<Mutex<Box<dyn Store>>>;

pub trait Store: Send + Sync {
    fn get(&self, key: &String) -> Result<Vec<u8>>;
    fn set(&mut self, key: String, val: Option<Vec<u8>>) -> Result<()>;
}

pub fn make_store() -> Box<dyn Store> {
    Box::new(DregStore {
        store: HashMap::<String, Vec<u8>>::new(),
    })
}

pub struct DregStore {
    store: HashMap<String, Vec<u8>>,
}

impl Store for DregStore {
    fn get(&self, key: &String) -> Result<Vec<u8>> {
        match self.store.get(key) {
            Some(v) => Ok(v.to_owned()),
            None => Err(Error::Custom("Value not found".to_string())),
        }
    }

    fn set(&mut self, key: String, val: Option<Vec<u8>>) -> Result<()> {
        match val {
            Some(v) => self.store.insert(key, v),
            None => self.store.remove(&key),
        };
        Ok(())
    }
}
