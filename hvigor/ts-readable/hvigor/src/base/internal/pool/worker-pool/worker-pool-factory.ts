/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/worker-pool/worker-pool-factory.js
 */
import * as global_core_parameters_js_1 from "../../data/global-core-parameters";
import * as worker_pool_type_js_1 from "../enum/worker-pool-type";
import * as worker_pool_impl_js_1 from "./worker-pool-impl";
export class WorkerPoolFactory {
  static getWorkerPool(o, r) {
    if (o === worker_pool_type_js_1.WorkerPoolType.LOCAL) return worker_pool_impl_js_1.WorkerPoolImpl.getInstance(r);
  }
  static getDefaultWorkerPool() {
    return this.getWorkerPool(worker_pool_type_js_1.WorkerPoolType.LOCAL, {
      minPoolNum: 2,
      maxPoolNum: global_core_parameters_js_1.coreParameter.properties.hvigorPoolMaxSize,
      maxCoreSize: global_core_parameters_js_1.coreParameter.properties.hvigorPoolMaxCoreSize,
      cacheCapacity: global_core_parameters_js_1.coreParameter.properties.hvigorPoolCacheCapacity,
      cacheTtl: global_core_parameters_js_1.coreParameter.properties.hvigorPoolCacheTtl
    });
  }
}
