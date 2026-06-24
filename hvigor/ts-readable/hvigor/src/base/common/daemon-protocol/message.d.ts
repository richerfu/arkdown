/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/common/daemon-protocol/message.d.ts
 */
/// <reference types="node" />
import { BuildStatus, OutputType } from './common-enum';
export type Message = object;
export interface OutputMessage extends Message {
    type: OutputType;
    text: string | Uint8Array;
    encoding?: BufferEncoding;
}
export declare function createOutput(type: OutputType, text: string | Uint8Array, encoding?: BufferEncoding): OutputMessage;
export interface BuildStatusMessage extends Message {
    status: BuildStatus;
    exitCode?: number;
    reason?: string;
}
export declare function createBuildStatus(status: BuildStatus, exitCode?: number, reason?: string): BuildStatusMessage;
export interface Command extends Message {
    reason: string;
}
export type CancelBuildCommand = Command;
export type StopDaemonCommand = Command;
