/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-compile-node.d.ts
 */
import { FileSet, TaskDetails } from '@ohos/hvigor';
import { CodeType } from '../../enum/code-type-enum';
import { OhosLogger } from '../../utils/log/ohos-logger';
import { CompileNode } from '../compile-node';
import { TargetTaskService } from '../service/target-task-service';
/**
 * 调用loader编译老模型的js/ets源码
 *
 * @since 2022/1/20
 */
export declare class LegacyCompileNode extends CompileNode {
    protected log: OhosLogger;
    protected readonly manifestDir: string;
    protected readonly resourceTablePath: string;
    protected readonly rawFileResource: string;
    constructor(taskService: TargetTaskService, codeType: CodeType, taskDetails?: TaskDetails);
    declareOutputFiles(): FileSet;
    declareInputFiles(): FileSet;
    initTaskDepends(): void;
    taskShouldDo(): boolean;
    protected doTaskAction(): Promise<void>;
    private compileTestRunner;
    /**
     * 判断是否需要编译testRunner
     *
     * @returns {[boolean, string | undefined]} 判断结果
     */
    protected needCompileTestRunner(): [boolean, string | undefined];
}
