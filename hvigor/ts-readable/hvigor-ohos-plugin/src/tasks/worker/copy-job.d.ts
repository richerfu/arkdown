/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/worker/copy-job.d.ts
 */
import { CopyConfig } from '../../utils/copy-resources-util';
/**
 * 指定要copy的模式
 */
export interface PatternObjType {
    src: string;
    dest: string;
    ignore?: Ignore;
    onlyFiles?: boolean;
    rename?: (name: string, extension: string, fullPath: string) => string;
}
interface Ignore {
    ignored: (p: {
        path: string;
        name: string;
    }) => boolean;
}
interface Target {
    src: string;
    dest: string;
    rename?: string | Function;
    ignore?: Ignore;
    onlyFiles?: boolean;
}
export interface CopyOptions {
    targets?: Target[];
}
export declare function copyByCopyOptions(config: CopyConfig): Promise<void>;
export {};
