/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/process/daemon-process-factory.d.ts
 */
import { DaemonInfo } from '../registry/daemon-info';
/**
 * 生成新daemon process的工厂类
 */
export declare class DaemonProcessFactory {
    private readonly childExecArgv;
    createDaemonPromise: Promise<void> | undefined;
    constructor();
    getProjectCompatibleIdleDaemon(needAwait?: boolean): Promise<DaemonInfo | undefined>;
    private killOldProcessAndCreateNew;
    /**
     * 检查状态为busy但实际上已经停止的daemon，并更新它们的状态
     * @returns 被清理的daemon数量
     */
    private cleanupBrokenBusyDaemons;
    /**
     * 获取所有状态为busy的daemon
     * @returns busy状态的所有daemon信息
     */
    private getBusyDaemons;
    private createNewIdleDaemon;
    private getWaitConnectTimeout;
    /**
     * 新启动一个node进程去创建daemon
     */
    createDaemonFork(): void;
}
export declare const defaultDaemonServerFactory: DaemonProcessFactory;
