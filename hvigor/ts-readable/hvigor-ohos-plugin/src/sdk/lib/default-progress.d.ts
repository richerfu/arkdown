/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/sdk/lib/default-progress.d.ts
 */
import { Progress } from '@ohos/sdkmanager-common';
export declare class DefaultProgress implements Progress {
    debug(log: string): void;
    error(log: string): void;
    fatal(log: string): void;
    info(log: string): void;
    warn(log: string): void;
}
