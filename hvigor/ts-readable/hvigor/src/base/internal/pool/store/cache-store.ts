/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/store/cache-store.js
 */
import * as const_js_1 from "../../../../common/const/const";
import * as hvigor_config_loader_js_1 from "../../../../common/util/hvigor-config-loader";
export class CacheStore {
  constructor(e, t, s = false) {
    this.caches = new Map(), this.hvigorMemoryThreshold = hvigor_config_loader_js_1.HvigorConfigLoader.getInstance().getPropertiesConfigValue(const_js_1.HVIGOR_MEMORY_THRESHOLD), this.hvigorEnableMemoryCache = hvigor_config_loader_js_1.HvigorConfigLoader.getInstance().getPropertiesConfigValue(const_js_1.HVIGOR_ENABLE_MEMORY_CACHE), this.name = e, this.lruCache = t, this.pinned = s;
  }
  getCache(e) {
    return this.lruCache.access(this.completeKey(e)), this.caches.get(e);
  }
  get(e) {
    return this.getCache(e);
  }
  set(e, t) {
    return this.setCache(e, t);
  }
  setCache(e, t, s = false) {
    if (this.hvigorMemoryThreshold && this.hvigorMemoryThreshold > 0 || false === this.hvigorEnableMemoryCache) return this;
    this.caches.has(e);
    if (this.caches.set(e, t), s || this.pinned) return this;
    const h = this.lruCache.set(this.completeKey(e));
    return h && this.caches.delete(this.parseCompleteKey(h)), this;
  }
  hasCache(e) {
    return this.caches.has(e);
  }
  clear() {
    this.caches.forEach((e, t) => {
      this.lruCache.delete(this.completeKey(t));
    }), this.caches.clear();
  }
  delete(e) {
    return this.lruCache.delete(this.completeKey(e)), this.caches.delete(e);
  }
  size() {
    return this.caches.size;
  }
  keys() {
    return this.caches.keys();
  }
  completeKey(e) {
    return `${this.name}@${e}`;
  }
  parseCompleteKey(e) {
    return e.substring(this.name.length + 1);
  }
}
export class CacheStoreManager {
  constructor(e) {
    this.cacheStores = new Map(), this.lruCache = new LRUCache(e);
  }
  mount(e, t = false) {
    return this.cacheStores.has(e) || this.cacheStores.set(e, new CacheStore(e, this.lruCache, t)), this.cacheStores.get(e);
  }
  unmount(e) {
    var t;
    return null === (t = this.cacheStores.get(e)) || undefined === t || t.clear(), this.cacheStores.delete(e);
  }
  clear() {
    this.lruCache.clear(), this.cacheStores.clear();
  }
  size() {
    return this.cacheStores.size;
  }
  keys() {
    return this.cacheStores.keys();
  }
  cacheItemSize() {
    return Array.from(this.cacheStores.values()).reduce((e, t) => e + t.size(), 0);
  }
}
class LinkedNode {
  constructor(e) {
    this.key = e, this.birth = Date.now();
  }
}
class LRUCache {
  constructor(e) {
    var t, s;
    this.cnt = 0, this.capacity = null !== (t = null == e ? undefined : e.capacity) && undefined !== t ? t : 4, this.ttl = null !== (s = null == e ? undefined : e.ttl) && undefined !== s ? s : 36e5, this.key2node = new Map(), this.head = new LinkedNode(""), this.tail = new LinkedNode(""), this.head.next = this.tail, this.ttl && this.ttl < 0 && (this.ttl = undefined), this.capacity && this.capacity < 0 && (this.capacity = undefined);
  }
  access(e) {
    const t = this.key2node.get(e);
    t && this.moveToHead(t);
  }
  set(e) {
    const t = this.key2node.get(e);
    if (t) return void this.moveToHead(t);
    const s = new LinkedNode(e),
      h = this.head.next;
    return this.head.next = s, s.next = h, s.pre = this.head, h.pre = s, this.key2node.set(e, s), this.cnt++, this.eliminate();
  }
  delete(e) {
    const t = this.key2node.get(e);
    t && (t.pre.next = t.next, t.next.pre = t.pre, this.key2node.delete(e), this.cnt--);
  }
  moveToHead(e) {
    e.pre.next = e.next, e.next.pre = e.pre;
    const t = this.head.next;
    this.head.next = e, e.next = t, e.pre = this.head, t.pre = e;
  }
  eliminate() {
    if (undefined === this.capacity || this.cnt <= this.capacity) return;
    const e = this.tail.pre;
    return this.key2node.delete(e.key), e.pre.next = this.tail, this.tail.pre = e.pre, this.cnt--, e.key;
  }
  clear() {
    this.key2node.clear(), this.cnt = 0, this.head.next = this.tail, this.tail.pre = this.tail.next = this.head.pre = undefined;
  }
  getTTL() {
    return this.ttl;
  }
}
