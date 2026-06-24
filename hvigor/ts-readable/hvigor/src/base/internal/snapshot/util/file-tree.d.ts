/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/snapshot/util/file-tree.d.ts
 */
import { FileType } from './file-type';
/**
 * File Visit options
 */
export interface FileOptions {
    maxDepth?: number;
    normalizePath?: boolean;
    filter?: RegExp;
    extensions?: RegExp;
    exclude?: RegExp;
    followSymlinks?: boolean;
}
export declare class FileInfo {
    name: string;
    path: string;
    type: FileType;
    size: number;
    lastModifiedTime: number;
    isSymbolicLink: boolean;
    constructor(name: string, path: string, type?: FileType, size?: number, lastModifiedTime?: number, isSymbolicLink?: boolean);
    toString(): string;
}
export interface FileVisitor {
    preVisitDirectory(fileInfo: FileInfo): void;
    visitFile(fileInfo: FileInfo): void;
    visitFileFailed(path: string, error: any): void;
    postVisitDirectory(fileInfo: FileInfo): void;
}
/**
 * Walk file tree in depth first recursive way
 *
 * @param path file or directory path
 * @param fileVisitor file visitor (depth first)
 * @param options file options
 */
export declare function walkFileTree(path: string, fileVisitor: FileVisitor, options?: FileOptions): void;
