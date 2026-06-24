/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/metrics/event/counter-event.d.ts
 */
import { BaseEvent, EventAdditional } from './base-event';
export declare class CounterEventAdditional implements EventAdditional {
    success: number;
    failed: number;
    constructor(success?: number, failed?: number);
}
export declare class CounterEvent extends BaseEvent {
    additional: CounterEventAdditional;
    constructor(id: string, name: string, description: string, pid: number, tid: string, success?: number, failed?: number);
}
