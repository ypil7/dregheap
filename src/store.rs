use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub type DregStore = Arc<Mutex<HashMap<String, Vec<u8>>>>;

pub fn new_store() -> DregStore {
    Arc::new(Mutex::new(HashMap::new()))
}
