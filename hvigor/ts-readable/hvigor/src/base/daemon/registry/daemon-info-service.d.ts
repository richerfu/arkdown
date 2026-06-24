/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/daemon/registry/daemon-info-service.d.ts
 */
/// <reference types="node" />
import { AddressInfo } from 'net';
import { DaemonInfo } from './daemon-info';
declare class DaemonInfoService {
    createDefaultDaemonInfo(serverAddress: AddressInfo, sessionId: string): DaemonInfo;
}
export declare const defaultDaemonInfoService: DaemonInfoService;
export {};
