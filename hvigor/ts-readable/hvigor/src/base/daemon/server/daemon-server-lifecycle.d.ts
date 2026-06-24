/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/server/daemon-server-lifecycle.d.ts
 */
import { HvigorDaemonServer } from './hvigor-daemon-server';
export declare class DaemonServerLifecycle {
    /**
     * server启动并监听端口
     */
    startListen(hvigorDaemonServer: HvigorDaemonServer): Promise<void>;
}
