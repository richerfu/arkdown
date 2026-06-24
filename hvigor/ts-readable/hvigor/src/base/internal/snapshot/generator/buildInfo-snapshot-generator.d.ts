/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/generator/buildInfo-snapshot-generator.d.ts
 */
import { Snapshot } from '../core/snapshot';
/**
 * 构建信息快照生成器
 *
 */
export interface BuildInfoSnapshotGenerator {
    /**
     * 根据json字符串生成快照map
     *
     * @param text
     */
    loadSnapshotCacheFromJson(text: string): Map<string, Snapshot>;
    /**
     * 将快照map序列化为json字符串
     */
    serializeSnapshotCacheToJson(): string;
}
