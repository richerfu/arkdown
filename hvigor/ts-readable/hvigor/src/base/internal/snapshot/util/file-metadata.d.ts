/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/util/file-metadata.d.ts
 */
export declare class FileMetaData {
    size: number;
    lastModifiedTime: number;
    constructor(size?: number, lastModifiedTime?: number);
    equals(other: FileMetaData): boolean;
}
