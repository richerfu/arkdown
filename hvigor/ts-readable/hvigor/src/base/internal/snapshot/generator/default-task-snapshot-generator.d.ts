/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/generator/default-task-snapshot-generator.d.ts
 */
import { IncrementalTask } from '../../../external/task/incremental-task';
import { TaskSnapshot } from '../core/task-snapshot';
import { TaskSnapshotGenerator } from './task-snapshot-generator';
export declare class DefaultTaskSnapshotGenerator implements TaskSnapshotGenerator {
    snapshot(task: IncrementalTask): TaskSnapshot;
    loadSnapshotFromJson(text: string): TaskSnapshot;
    loadSnapshotCacheFromJson(text: string): Map<string, TaskSnapshot>;
    serializeSnapshotCacheToJson(collection: Map<string, TaskSnapshot>): string;
}
