/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/metrics/event/gauge-event.d.ts
 */
import { BaseEvent, EventAdditional } from './base-event';
export declare class GaugeEventAdditional implements EventAdditional {
    utilization: number;
    constructor(utilization: number);
}
export declare class GaugeEvent extends BaseEvent {
    additional: GaugeEventAdditional;
    constructor(id: string, name: string, description: string, pid: number, tid: string, utilization: number);
}
