/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/worker-pool/worker-pool-factory.d.ts
 */
import { WorkerPoolType } from '../enum/worker-pool-type';
import { PoolConfig } from './pool-config';
import { WorkerPool } from './worker-pool';
/**
 * 工作池工厂
 * 创建不同类型的工作池
 *
 * @since 2022/8/17
 */
export declare class WorkerPoolFactory {
    static getWorkerPool(type: WorkerPoolType, poolConfig: PoolConfig): WorkerPool | undefined;
    static getDefaultWorkerPool(): WorkerPool;
}
