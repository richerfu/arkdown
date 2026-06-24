/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/metrics/event/object-event.d.ts
 */
import { BaseEvent, EventAdditional } from './base-event';
export declare enum ObjectEventState {
    NEW = "new",
    SNAPSHOT = "snapshot",
    DESTROY = "destroy"
}
export declare class ObjectEventAdditional implements EventAdditional {
    objectId: string;
    state: ObjectEventState;
    snapshot?: object;
    constructor(objectId: string, state: ObjectEventState, snapshot?: object);
}
export declare class ObjectEvent extends BaseEvent {
    additional: ObjectEventAdditional;
    constructor(id: string, name: string, description: string, pid: number, tid: string, objectId: string, state: ObjectEventState, snapshot?: object);
}
