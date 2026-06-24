/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-ohos-test-compile-node.d.ts
 */
import { FileSet } from '@ohos/hvigor';
import { CodeType } from '../../enum/code-type-enum';
import { TargetTaskService } from '../service/target-task-service';
import { LegacyCompileNode } from './legacy-compile-node';
/**
 * 处理ohosTest Target时调用loader编译老模型的js/arkts源码
 *
 * @since 2022/12/22
 */
export declare class LegacyOhosTestCompileNode extends LegacyCompileNode {
    constructor(taskService: TargetTaskService, codeType: CodeType);
    declareInputFiles(): FileSet;
}
