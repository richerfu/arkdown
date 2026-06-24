/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/har/har-compile-node.d.ts
 */
import { FileSet, TaskDetails } from '@ohos/hvigor';
import { NodeCommandBuilder } from '../../builder/node-command-builder';
import { CodeType } from '../../enum/code-type-enum';
import { CompileNode } from '../compile-node';
import { TargetTaskService } from '../service/target-task-service';
export declare enum ObfuscateType {
    UGLIFY = "uglify"
}
/**
 * Har编译arkTs相关源码
 */
export declare class HarCompileNode extends CompileNode {
    constructor(taskService: TargetTaskService, codeType: CodeType, taskDetails: TaskDetails);
    declareInputFiles(): FileSet;
    protected doTaskAction(): Promise<void>;
    protected createNodeCommandBuilder(): NodeCommandBuilder;
}
