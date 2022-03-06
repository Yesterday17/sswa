use std::collections::HashMap;
use lazy_static::lazy_static;
use parking_lot::RwLock;

lazy_static! {
    pub static ref CONTEXT: Context = Context::new();
}

pub struct Context(pub(crate) RwLock<HashMap<String, String>>);

impl Context {
    pub fn new() -> Self {
        Self(RwLock::new(HashMap::new()))
    }

    pub fn insert_sys<S>(&self, mut key: String, value: S)
        where S: Into<String> {
        let value = value.into();
        self.insert(key.clone(), value.clone());

        // 兼容考虑的 ss_{var}
        key.insert_str(0, "ss_");
        self.0.write().insert(key, value);
    }

    pub fn insert<S>(&self, key: String, value: S)
        where S: Into<String> {
        self.0.write().insert(key, value.into());
    }

    pub fn get(&self, key: &str) -> anyhow::Result<String> {
        let me = self.0.read();
        Ok(me.get(key)
            .map(|value| value.into())
            .ok_or_else(|| anyhow::anyhow!("{} not found", key))?
        )
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.0.read().contains_key(key)
    }
}