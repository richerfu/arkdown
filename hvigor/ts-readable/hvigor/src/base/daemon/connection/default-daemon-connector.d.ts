/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/connection/default-daemon-connector.d.ts
 */
import { Socket } from 'socket.io-client';
import { DaemonInfo } from '../registry/daemon-info';
import { DaemonConnector } from './daemon-connector';
/**
 * 为client获取可用的与daemon的socket连接
 */
export declare class DefaultDaemonConnector implements DaemonConnector {
    getDaemonConnection(): Promise<Socket | undefined>;
    onConnect(...args: any[]): void;
    onClose(daemonInfo: DaemonInfo, reason: string): void;
    onConnectError(daemonInfo: DaemonInfo, error: Error): void;
}
