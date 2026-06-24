/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/vigor/task/interface/input-output.d.ts
 */
import { FileSet } from '../../../internal/snapshot/util/file-set';
import { TaskInputValue } from '../../../internal/snapshot/util/task-input-value-entry';
export declare class TaskInput {
    private declareInputs;
    private declareInputFiles;
    file(path: string): TaskInput;
    private _file;
    files(paths: string[]): TaskInput;
    private _files;
    property(key: string, value: TaskInputValue): TaskInput;
    private _property;
}
export declare class TaskOutput {
    private declareOutputFiles;
    constructor(declareOutputFiles: FileSet);
    file(path: string): TaskOutput;
    private _file;
    files(paths: string[]): TaskOutput;
    private _files;
}
