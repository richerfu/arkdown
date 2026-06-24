/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/util/build-result.d.ts
 */
import { SerializedTimings } from 'rollup';
export interface BuildResult {
    timing?: SerializedTimings;
    callback?: (isBuildSuccess: boolean) => Promise<void> | void;
}
