/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/worker-manager/worker-manager-impl.d.ts
 */
import { Level } from 'log4js';
import { Work } from '../model/work';
import { PoolConfig } from '../worker-pool/pool-config';
import { WorkerManager } from './worker-manager';
/**
 * worker管理器的实现类
 *
 * @since 2022/8/15
 */
export declare class WorkerManagerImpl implements WorkerManager {
    private _log;
    private readonly maxPoolNum;
    private readonly minPoolNum;
    private readonly idleWorkers;
    private readonly busyWorkers;
    private readonly dispatch;
    private readonly addLog;
    private readonly maxCoreSize?;
    private readonly residentWorkers;
    private readonly cacheCapacity?;
    private readonly cacheTtl?;
    private readonly hvigorMemoryThreshold;
    private recycleInterval;
    private maxIdleTime;
    private deadWorkers;
    private timer;
    private callbacks;
    private errorCallback;
    private eventMap;
    constructor(poolConfig: PoolConfig, dispatch: Function, addLog: Function);
    getMaxPoolNum(): number;
    getMinPoolNum(): number;
    updateLogLevel(newLogLevel: Level): void;
    cleanUp(callback: () => void): void;
    clear(callback: Function): void;
    /**
     * 构建结束时，每个线程HVIGOR_CONFIG_LOADER进行清理
     */
    private cleanHvigorConfig;
    /**
     * 构建结束时，每个线程HVIGOR_LOGGER进行清理
     */
    private cleanHvigorLogger;
    /**
     * 清空worker的map集合
     *
     * @param callback 清空后的回调函数
     * @param workers 待清空的worker的map集合
     */
    private clearWorkers;
    createWorker(workerId?: number, preludeDeps?: string[]): boolean;
    private listenWorkerEvents;
    private listenExitEvent;
    private listenErrorEvent;
    private listenMessageEvent;
    private workerOnMessage;
    private onWorkError;
    private _createWorker;
    /**
     * 获取创建worker时分配的id
     *
     * @param expectedId 希望被分配的id
     * @returns {number} 被分配的id
     */
    private getAllocatedId;
    /**
     * 通知执行引擎工作单元执行完毕的事件
     *
     * @param id 工作单元id
     * @param event 待通知的事件
     */
    private notify;
    private notifyError;
    /**
     * 获取日志数据处理函数
     *
     * @param hWorker 需要处理的日志worker对象
     * @returns {Function} 日志数据处理函数
     */
    private getDataHandler;
    doWork(work: Work, callback: Function, callbackInput: unknown[], workerId: number): boolean;
    getAvailableWorkers(): number[];
    hasAvailableWorkers(): boolean;
    recycle(): void;
    setMaxIdleTime(maxIdleTime: number): void;
    setRecycleInterval(recycleInterval: number): void;
    setErrorCallback(errorCallback: Function): void;
    isBusyWorker(workerId: number): boolean;
    isIdleWorker(workerId: number): boolean;
    isRecycledWorker(workerId: number): boolean;
    isFull(): boolean;
}
