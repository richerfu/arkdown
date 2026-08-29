use std::hash::{Hash, Hasher};

use indexmap::IndexMap;
use napi_derive::napi;

#[napi(object)]
pub struct RuntimeHvigorCacheConfig {
  pub capacity: Option<i64>,
  pub ttl: Option<i64>,
}

#[napi(object)]
pub struct RuntimeHvigorCacheEviction {
  pub store_name: String,
  pub key: String,
}

#[derive(Clone, Eq)]
struct CacheIdentity {
  store_name: String,
  key: String,
}

impl PartialEq for CacheIdentity {
  fn eq(&self, other: &Self) -> bool {
    self.store_name == other.store_name && self.key == other.key
  }
}

impl Hash for CacheIdentity {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.store_name.hash(state);
    self.key.hash(state);
  }
}

/// Rust-owned shared LRU index behind Hvigor's public CacheStoreManager.
/// Arbitrary cached JavaScript values remain in V8 and are addressed only by
/// the store/key identity selected here.
#[napi]
pub struct RuntimeHvigorCacheStoreManager {
  capacity: Option<usize>,
  ttl: Option<i64>,
  enabled: bool,
  stores: IndexMap<String, bool>,
  lru: IndexMap<CacheIdentity, ()>,
}

#[napi]
impl RuntimeHvigorCacheStoreManager {
  #[napi(constructor)]
  pub fn new(config: Option<RuntimeHvigorCacheConfig>, enabled: Option<bool>) -> Self {
    let config = config.unwrap_or(RuntimeHvigorCacheConfig {
      capacity: None,
      ttl: None,
    });
    Self {
      capacity: match config.capacity.unwrap_or(4) {
        value if value < 0 => None,
        value => Some(value as usize),
      },
      ttl: match config.ttl.unwrap_or(3_600_000) {
        value if value < 0 => None,
        value => Some(value),
      },
      enabled: enabled.unwrap_or(true),
      stores: IndexMap::new(),
      lru: IndexMap::new(),
    }
  }

  #[napi]
  pub fn mount(&mut self, store_name: String, pinned: Option<bool>) -> bool {
    if self.stores.contains_key(&store_name) {
      return false;
    }
    self.stores.insert(store_name, pinned.unwrap_or(false));
    true
  }

  #[napi]
  pub fn unmount(&mut self, store_name: String) -> bool {
    let removed = self.stores.shift_remove(&store_name).is_some();
    self
      .lru
      .retain(|identity, _| identity.store_name != store_name);
    removed
  }

  #[napi]
  pub fn access(&mut self, store_name: String, key: String) {
    let identity = CacheIdentity { store_name, key };
    if self.lru.shift_remove(&identity).is_some() {
      self.lru.insert(identity, ());
    }
  }

  #[napi]
  pub fn set(
    &mut self,
    store_name: String,
    key: String,
    pinned: Option<bool>,
  ) -> Option<RuntimeHvigorCacheEviction> {
    if !self.enabled {
      return None;
    }
    let store_pinned = self.stores.get(&store_name).copied().unwrap_or(false);
    if store_pinned || pinned.unwrap_or(false) {
      return None;
    }
    let identity = CacheIdentity { store_name, key };
    if self.lru.shift_remove(&identity).is_some() {
      self.lru.insert(identity, ());
      return None;
    }
    self.lru.insert(identity, ());
    let capacity = self.capacity?;
    if self.lru.len() <= capacity {
      return None;
    }
    self
      .lru
      .shift_remove_index(0)
      .map(|(identity, ())| RuntimeHvigorCacheEviction {
        store_name: identity.store_name,
        key: identity.key,
      })
  }

  #[napi]
  pub fn delete(&mut self, store_name: String, key: String) {
    self.lru.shift_remove(&CacheIdentity { store_name, key });
  }

  #[napi]
  pub fn clear(&mut self) {
    self.lru.clear();
    self.stores.clear();
  }

  #[napi(getter)]
  pub fn enabled(&self) -> bool {
    self.enabled
  }

  #[napi(getter)]
  pub fn ttl(&self) -> Option<i64> {
    self.ttl
  }
}
