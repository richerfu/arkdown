/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/util/fs.d.ts
 */
/// <reference types="node" />
import fs from 'fs';
export declare const access: typeof fs.access.__promisify__;
export declare const readFile: typeof fs.readFile.__promisify__;
export declare const stat: typeof fs.stat.__promisify__;
export declare function clearRealpathCache(): void;
export declare function clearFileCache(): void;
export declare function enableRealpathCache(): void;
export declare function disableRealpathCache(): void;
export declare function fileExists(filePath: fs.PathLike): Promise<boolean>;
export declare function checkFileExistsAndSize(filePath: string): Promise<boolean | undefined>;
export declare function resolveSymlink(path: string): Promise<string | undefined>;
export declare function realpathFS(path: string): Promise<string | undefined>;
export declare function transformPath(p: string): string;
