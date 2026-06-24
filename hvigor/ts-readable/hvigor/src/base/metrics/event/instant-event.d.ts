/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/metrics/event/instant-event.d.ts
 */
import { BaseEvent, EventAdditional } from './base-event';
export declare enum InstantEventScope {
    THREAD = "thread",
    PROCESS = "process",
    GLOBAL = "global"
}
export declare class InstantEventAdditional implements EventAdditional {
    scope?: InstantEventScope;
}
export declare class InstantEvent extends BaseEvent {
    additional: InstantEventAdditional;
    constructor(id: string, name: string, description: string, pid: number, tid: string);
    setScope(scope: InstantEventScope): void;
}
