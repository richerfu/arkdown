/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/service/snapshot-comparator-service.d.ts
 */
import { Snapshot } from '../core/snapshot';
export declare class SnapshotComparatorService {
    private static _instance;
    static getInstance(): SnapshotComparatorService;
    static isDiff(oldSnapShot: Snapshot | undefined, newSnapShot: Snapshot | undefined): boolean;
}
