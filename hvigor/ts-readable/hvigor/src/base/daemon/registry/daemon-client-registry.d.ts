/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/registry/daemon-client-registry.d.ts
 */
import { DaemonInfo, DaemonState } from './daemon-info';
import { DefaultDaemonRegistry } from './default-daemon-registry';
declare class DaemonClientRegistry extends DefaultDaemonRegistry {
    constructor();
    updateDaemonProcessState(daemonInfo: DaemonInfo, daemonState: DaemonState, stateChangeInfo?: string): void;
    executeAfterChangeProcessStateInClient(daemonInfo: DaemonInfo, daemonState: DaemonState): void;
    /**
     * 先根据工程路径找对应的注册表中是否有对应的信息
     *
     * @returns {DaemonInfo | undefined}
     */
    findProjectDaemon(): DaemonInfo | undefined;
    /**
     * 注册表中可用状态的daemon是否超过上限
     *
     * @returns {boolean}
     */
    isAvailableDaemonOverLimit(): boolean;
    /**
     * 找一个最久未使用的Idle状态的Daemon process
     *
     * @returns {DaemonInfo | undefined}
     */
    findIdleDaemonWithLRU(): DaemonInfo | undefined;
}
export declare const defaultDaemonClientRegistry: DaemonClientRegistry;
export {};
