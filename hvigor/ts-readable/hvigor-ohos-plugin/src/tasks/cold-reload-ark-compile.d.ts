/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/cold-reload-ark-compile.d.ts
 */
import { FileSet } from '@ohos/hvigor';
import { OhosLogger } from '../utils/log/ohos-logger';
import { ArkCompile } from './ark-compile';
export declare class ColdReloadArkCompile extends ArkCompile {
    logger: OhosLogger;
    taskShouldDo(): boolean;
    declareInputFiles(): FileSet;
    protected doTaskAction(): Promise<void>;
    protected allowToPreloadSystemSo(): boolean;
    private getColdReloadPatchConfig;
    initTaskDepends(): void;
}
