/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/metrics/event/metadata-event.d.ts
 */
import { BaseEvent, EventAdditional } from './base-event';
export declare enum MetadataEventState {
    NEW = "new",
    IDLE = "idle",
    BUSY = "busy",
    CLOSE = "close",
    BROKEN = "broken"
}
declare class MetadataEventAdditional implements EventAdditional {
    category?: string;
    sortIndex?: string;
    label?: string;
    content?: string;
    state: MetadataEventState;
    constructor(state: MetadataEventState);
}
export declare class MetadataEvent extends BaseEvent {
    additional: MetadataEventAdditional;
    constructor(id: string, name: string, description: string, pid: number, tid: string, state: MetadataEventState);
    setCategory(category: string): void;
    setSortIndex(sortIndex: string): void;
    setLabel(label: string): void;
    setContent(content: string): void;
}
export {};
