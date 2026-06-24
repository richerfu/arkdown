/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/cache/service/metric-cache-service.js
 */
import * as map_cache_service_js_1 from "./map-cache-service";
export class MetricCacheService extends map_cache_service_js_1.MapCacheService {
  constructor() {
    super();
  }
  add(e) {
    this.cacheEntryMap.set(e.getId(), e);
  }
  getEvents() {
    const e = [];
    return this.cacheEntryMap.forEach(c => {
      e.push(c);
    }), e;
  }
  static getInstance() {
    return MetricCacheService.instance || (MetricCacheService.instance = new MetricCacheService()), MetricCacheService.instance;
  }
}
