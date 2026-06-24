/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/dispatcher/loop-priority-dispatcher.d.ts
 */
import { LogServiceImpl } from '../log-collection/log-service-impl';
import { ReadyQueue } from '../ready-queue/ready-queue';
import { WorkerManager } from '../worker-manager/worker-manager';
import { Dispatcher } from './dispatcher';
/**
 * 每次遍历worker进行分发的优先级分发器
 *
 * @since 2022/8/24
 */
export declare class LoopPriorityDispatcher implements Dispatcher {
    private failedAttempts;
    dispatch(logService: LogServiceImpl, readyQueue: ReadyQueue, workerManager: WorkerManager, workerId?: number): boolean;
    private dispatchWithGivenWorker;
}
