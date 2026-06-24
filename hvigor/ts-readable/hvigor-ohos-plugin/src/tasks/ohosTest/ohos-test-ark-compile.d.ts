/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/ohosTest/ohos-test-ark-compile.d.ts
 */
import { FileSet } from '@ohos/hvigor';
import { CodeType } from '../../enum/code-type-enum';
import { TargetTaskService } from '../service/target-task-service';
import { ProjectConfig } from '@ohos/hvigor-arkts-compose';
import { ArkCompile } from '../ark-compile';
/**
 * 处理ohosTest Target时调用loader编译新模型的js/ets源码
 *
 * @since 2023/1/28
 */
export declare class OhosTestArkCompile extends ArkCompile {
    constructor(taskService: TargetTaskService, codeType: CodeType);
    declareInputFiles(): FileSet;
    protected allowToPreloadSystemSo(): boolean;
    protected initDefaultArkCompileConfig(): Promise<ProjectConfig>;
}
