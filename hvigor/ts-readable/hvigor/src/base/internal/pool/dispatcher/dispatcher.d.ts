/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/dispatcher/dispatcher.d.ts
 */
import { LogServiceImpl } from '../log-collection/log-service-impl';
import { ReadyQueue } from '../ready-queue/ready-queue';
import { WorkerManager } from '../worker-manager/worker-manager';
/**
 * 分发器
 * 从就绪队列中获取工作单元，根据一定的策略分发到某个worker
 *
 * @since 2022/8/16
 */
export interface Dispatcher {
    /**
     * 分发工作单元
     *
     * @param logService 日志服务
     * @param readyQueue 就绪队列
     * @param workerManager worker管理器
     * @param workerId 希望被分发的worker的id
     * @returns {boolean} 分发是否成功
     */
    dispatch(logService: LogServiceImpl, readyQueue: ReadyQueue, workerManager: WorkerManager, workerId?: number): boolean;
}
