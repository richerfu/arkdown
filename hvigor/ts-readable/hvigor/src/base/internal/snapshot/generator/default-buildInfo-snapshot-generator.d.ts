/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/generator/default-buildInfo-snapshot-generator.d.ts
 */
import { BuildInfoSnapshotGenerator } from './buildInfo-snapshot-generator';
import { Snapshot } from '../core/snapshot';
/**
 *  构建信息快照生成器
 */
export declare class DefaultBuildInfoSnapshotGenerator implements BuildInfoSnapshotGenerator {
    loadSnapshotCacheFromJson(text: string): Map<string, Snapshot>;
    serializeSnapshotCacheToJson(): string;
}
