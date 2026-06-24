/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/fs/fs-event.d.ts
 */
/// <reference types="node" />
import fs from 'fs';
import { EventType } from './event-type';
export declare class FsEvent {
    private readonly name;
    private readonly file;
    private readonly eventType;
    private readonly stats;
    constructor(eventType: EventType, name: string, file: string, stats: fs.Stats);
    getName(): string;
    type(): EventType;
    getFile(): string;
    getStats(): fs.Stats;
}
