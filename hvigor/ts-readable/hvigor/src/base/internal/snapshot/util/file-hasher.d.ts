/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/util/file-hasher.d.ts
 */
import { FileMetaData } from './file-metadata';
export interface FileHasher {
    hash(filepath: string, fileMetaData?: FileMetaData): string;
}
export declare class FileContentHasher implements FileHasher {
    hash(filepath: string): string;
}
export declare class FileMetaHasher implements FileHasher {
    hash(filepath: string, fileMetaData: FileMetaData): string;
}
