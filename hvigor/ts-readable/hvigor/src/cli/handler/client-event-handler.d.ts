/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/cli/handler/client-event-handler.d.ts
 */
import { BuildStatusMessage, Message, OutputMessage } from '../../base/common/daemon-protocol/message';
import { HvigorLogger } from '../../base/log/hvigor-log';
export declare function printLog(output: OutputMessage): void;
export declare function handleBuildStatus(msg: BuildStatusMessage): void;
export declare function handleWatchBuildResult(msg: any): void;
export declare function handleWatchLog(msg: any): void;
export declare function handleServerMsgError(log: HvigorLogger, msg: Message): void;
