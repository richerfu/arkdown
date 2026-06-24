/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/external/task/incremental-task.d.ts
 */
import { FileSet } from '../../internal/snapshot/util/file-set';
import { TaskInputValue } from '../../internal/snapshot/util/task-input-value-entry';
import { DefaultTask } from './default-task';
export declare abstract class IncrementalTask extends DefaultTask {
    declareInputs(): Map<string, TaskInputValue>;
    declareInputFiles(): FileSet;
    declareOutputFiles(): FileSet;
    /**
     * 无论是否增量，总是执行此方法
     */
    beforeAlwaysAction(): Promise<void>;
}
