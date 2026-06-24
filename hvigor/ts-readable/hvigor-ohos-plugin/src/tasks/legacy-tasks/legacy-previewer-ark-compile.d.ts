/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-previewer-ark-compile.d.ts
 */
import { FileSet } from '@ohos/hvigor';
import { OhosLogger } from '../../utils/log/ohos-logger';
import { LegacyCompileNode } from './legacy-compile-node';
export declare class LegacyPreviewerArkCompile extends LegacyCompileNode {
    logger: OhosLogger;
    declareInputFiles(): FileSet;
    protected doTaskAction(): Promise<void>;
    initTaskDepends(): void;
}
