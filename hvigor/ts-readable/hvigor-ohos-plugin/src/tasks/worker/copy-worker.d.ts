/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/worker/copy-worker.d.ts
 */
import { CoreTask, WorkerPoolDelegator } from '@ohos/hvigor';
import { ProjectConfig } from '@ohos/hvigor-arkts-compose';
import { OhosLogger } from '../../utils/log/ohos-logger';
/**
 * 用来向线程池提交一个copy的任务 copy什么可以通过参数指定
 */
export declare class CopyWorker {
    /**
     * 向线程池提交执行copy的任务
     *
     * @param task task
     * @param workerPool 线程池
     * @param logger 日志
     * @param config 编译参数
     * @param callback 编译后的回调函数，其入参是void
     * @param targetWorkers 目标worker
     */
    static submitCopyResourcesWork(task: CoreTask, workerPool: WorkerPoolDelegator, logger: OhosLogger, config: ProjectConfig, callback?: Function, targetWorkers?: number[]): Promise<string | null>;
}
