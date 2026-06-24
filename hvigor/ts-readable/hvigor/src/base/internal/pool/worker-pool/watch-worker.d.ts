/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/worker-pool/watch-worker.d.ts
 */
/// <reference types="node" />
import { Worker, WorkerOptions } from 'worker_threads';
import { CoreTaskImpl } from '../../../external/task/core-task-impl';
import { DurationEvent } from '../../../metrics/event/duration-event';
interface CustomAction {
    customType: string;
    action: Function;
}
/**
 * 用于watch任务的线程管理
 *
 * @since 2022/12/7
 */
declare class WatchWorker {
    private readonly workerMap;
    constructor();
    getWorker(id: number): Worker | undefined;
    /**
     * 获取上一次WATCH_RESULT的结果. 如果WATCH_START已经触发，需要等待WATCH_RESULT消息
     * @param id
     */
    getLastWatchResult(id: number): Promise<boolean>;
    sendCompileMsgToWorker(): void;
    sendNewWatchMsgToProcess(workerId: number): void;
    createWorker(task: CoreTaskImpl, filename: string, hasCustomTerminate: boolean, callback: Function, options?: WorkerOptions, customAction?: CustomAction): string;
    /**
     * 注册监听处理CUSTOM_WORKER_REQUEST事件并根据消息执行对应的回调函数
     * @param worker
     * @param workerId
     */
    createCustomActionListener(worker: Worker, workerId: number): void;
    createWorkerOnMessageListener(worker: Worker, resolve: (value: unknown) => void, reject: (reason?: unknown) => void, task: CoreTaskImpl, workerId: number, callback: Function, subDurationEvent: DurationEvent): void;
    /**
     * 加工转发watchWorker的WATCH消息给父进程
     * @param workerId
     * @param response
     * @private
     */
    private forwardMessageToProcess;
    /**
     * 处理WATCH_START和WATCH_RESULT消息，记录Watch是否完成以及是否正确的结果，并fulfill异步等待的Promise
     * @param workerId
     * @param response
     * @private
     */
    private handleWatchStartAndResult;
    createWorkerOnErrorListener(worker: Worker, reject: (reason?: unknown) => void, task: CoreTaskImpl, workerId: number): void;
    /**
     * 如果有自定义终止逻辑则发送TERMINATE_WORKER信号到 worker，否则直接终止 worker
     */
    terminate(id: number): void;
    /**
     * 设置 id 对应的 WorkerItem 的 keepAlive 成员变量
     */
    private setAliveForWorker;
    /**
     * 若 id 对应的 WorkerItem 的 keepAlive 信息为 true 则消费掉
     */
    private consumeKeepAlive;
    private terminateWorker;
    /**
     * 监听主线程传递的来自socket的信息，传递给对应的worker线程
     * @private
     */
    private addListenersOnSessionManager;
}
/**
 * 普通worker，编译完成后直接terminate。
 * 目前用于预览时级联编译entry所依赖的所有hsp。
 */
declare class NormalWorker {
    private static runningThreadCnt;
    private static workQueue;
    private static promiseMap;
    private readonly maxWorkerNum;
    createWorker(task: CoreTaskImpl, filename: string, hasCustomTerminate: boolean, callback: Function, options?: WorkerOptions): string;
    reset(): void;
    private executeWorker;
    private processNextTask;
}
export declare const watchWorker: WatchWorker;
export declare const normalWorker: NormalWorker;
export {};
