use lazy_static::lazy_static;
use parking_lot::RwLock;
use serde_json::Value;
use std::collections::HashMap;

lazy_static! {
    pub static ref CONTEXT: Context = Context::new();
}

pub struct Context(pub(crate) RwLock<HashMap<String, Value>>);

impl Context {
    pub fn new() -> Self {
        Self(RwLock::new(HashMap::new()))
    }

    pub fn insert_sys<S>(&self, mut key: String, value: S)
    where
        S: Into<Value>,
    {
        let value = value.into();
        self.insert(format!("${key}"), value.clone());

        // 兼容考虑的 ss_{var}
        key.insert_str(0, "ss_");
        self.0.write().insert(key, value);
    }

    pub fn insert<S>(&self, key: String, value: S)
    where
        S: Into<Value>,
    {
        self.0.write().insert(key, value.into());
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.0.read().contains_key(key)
    }
}
