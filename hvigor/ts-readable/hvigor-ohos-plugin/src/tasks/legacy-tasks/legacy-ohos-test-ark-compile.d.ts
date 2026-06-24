/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/legacy-ohos-test-ark-compile.d.ts
 */
import { FileSet } from '@ohos/hvigor';
import { ProjectConfig } from '@ohos/hvigor-arkts-compose';
import { CodeType } from '../../enum/code-type-enum';
import { LegacyAbilityModel } from '../../model/ability/legacy-ability-model';
import { TargetTaskService } from '../service/target-task-service';
import { LegacyArkCompile } from './legacy-ark-compile';
/**
 * 处理ohosTest Target时调用loader编译老模型的js/ets源码
 *
 * @since 2023/1/28
 */
export declare class LegacyOhosTestArkCompile extends LegacyArkCompile {
    constructor(taskService: TargetTaskService, codeType: CodeType);
    declareInputFiles(): FileSet;
    protected initDefaultLegacyArkConfig(rOS: string, sInfo: string, abilityInfo: LegacyAbilityModel): ProjectConfig;
}
