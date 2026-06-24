/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/pool/worker-pool/worker-delegator.d.ts
 */
import { CoreTask } from '../../../external/task/core-task';
import { LogType } from '../enum/log-type';
import { PoolState } from '../enum/pool-state';
import { Log } from '../log-collection/log';
import { TCB } from '../model/tcb';
import { SubmitOption } from './submit-option';
/**
 * 工作池代理器
 * 内置于task的对象，提供对工作池功能的间接访问
 *
 * @since 2022/8/31
 */
export declare class WorkerPoolDelegator {
    private workerPool;
    private static isActive;
    private static needWarmUp;
    constructor();
    warmUp(warmUpPath: string): void;
    submit(task: CoreTask, workPath: string, submitOption?: SubmitOption): TCB;
    terminate(): Promise<boolean>;
    getState(): PoolState;
    setState(state: PoolState): void;
    setRecycleInterval(recycleInterval: number): void;
    setMaxIdleTime(maxIdleTime: number): void;
    setErrorCallback(errorCallback: Function): void;
    getLog(type: LogType): Set<Log> | undefined;
    clearLog(type?: LogType): void;
    getMaxPoolNum(): number;
    getMinPoolNum(): number;
    setResident(resident: boolean): void;
    isActive(): boolean;
}
