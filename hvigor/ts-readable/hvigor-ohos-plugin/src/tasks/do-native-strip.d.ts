/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/do-native-strip.d.ts
 */
import { TaskInputValue } from '@ohos/hvigor';
import { AbstractNativeStrip } from './abstract/abstract-native-strip';
import { TargetTaskService } from './service/target-task-service';
export declare class DoNativeStrip extends AbstractNativeStrip {
    private _log;
    declareInputs(): Map<string, TaskInputValue>;
    private get doBinxoNativeLibs();
    private get doStrippedNativeLibs();
    constructor(targetService: TargetTaskService);
    protected doTaskAction(): Promise<void>;
    initTaskDepends(): void;
}
