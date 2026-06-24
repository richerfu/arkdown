/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/generator/filesystem-snapshot-generator.d.ts
 */
import { BasicFileSnapshot } from '../core/filesystem-snapshot';
import { FsOptions } from '../util/file-set';
/**
 * FileSystem snapshot generator
 *
 * @since 2022/9/1
 */
export interface FileSystemSnapshotGenerator {
    /**
     * Snapshot a file or dir
     *
     * @param filepath
     * @param options FsOptions for filter file
     */
    snapshot(filepath: string, options?: FsOptions): BasicFileSnapshot;
    /**
     * load file snapshot form json string and return BasicFileSnapshot
     *
     * @param jsonStr
     */
    loadSnapshotFromJson(jsonStr: string): BasicFileSnapshot | undefined;
    /**
     * load file snapshot form json string and return a map
     *
     * @param jsonStr
     */
    loadSnapshotCacheFromJson(jsonStr: string): Map<string, BasicFileSnapshot> | undefined;
    /**
     * serialize snapshot map to json string
     *
     * @param map
     */
    serializeSnapshotToJson(map: Map<string, BasicFileSnapshot>): string;
}
