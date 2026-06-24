/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/registry/default-daemon-registry.d.ts
 */
import { HvigorLogger } from '../../log/hvigor-log';
import { DaemonInfo } from './daemon-info';
interface DaemonRegistry {
    getIdles(): DaemonInfo[];
    getInfos(): DaemonInfo[];
    getInfo(keyId: string): DaemonInfo | undefined;
    setInfo(keyId: string, daemonInfo: DaemonInfo): void;
}
export declare class DefaultDaemonRegistry implements DaemonRegistry {
    private _log;
    private daemonCacheService;
    constructor(log: HvigorLogger);
    getIdles(): DaemonInfo[];
    getAlive(): DaemonInfo[];
    getInfo(keyId: string): DaemonInfo | undefined;
    getInfos(): DaemonInfo[];
    getDaemonInfoByPid(pid: number): DaemonInfo | undefined;
    setInfo(keyId: string, daemonInfo: DaemonInfo): void;
}
export {};
