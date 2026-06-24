/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/hot-reload-ark-compile.d.ts
 */
import { ArkCompile } from './ark-compile';
export declare class HotReloadArkCompile extends ArkCompile {
    private readonly logger;
    protected doTaskAction(): Promise<void>;
    initTaskDepends(): void;
}
