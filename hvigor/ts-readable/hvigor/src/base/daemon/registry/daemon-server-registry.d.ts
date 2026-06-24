/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/registry/daemon-server-registry.d.ts
 */
import { DaemonState } from './daemon-info';
import { DefaultDaemonRegistry } from './default-daemon-registry';
declare class DaemonServerRegistry extends DefaultDaemonRegistry {
    constructor();
    updateDaemonProcessState(daemonState: DaemonState, stateChangeInfo?: string): void;
    getAllPortInUsed(): number[];
}
export declare const defaultDaemonServerRegistry: DaemonServerRegistry;
export {};
