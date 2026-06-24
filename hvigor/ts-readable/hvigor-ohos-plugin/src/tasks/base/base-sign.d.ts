/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/base/base-sign.d.ts
 */
import { FileSet } from '@ohos/hvigor';
import { TaskDetails } from '@ohos/hvigor';
import { TaskInputValue } from '@ohos/hvigor';
import { SignTypeEnum } from '../../enum/sign-type-enum';
import { ProjectBuildProfile } from '../../options/build/project-build-profile';
import { TargetTaskService } from '../service/target-task-service';
import { SignUtil } from '../sign/sign-util';
import { OhosHapTask } from '../task/ohos-hap-task';
import SigningConfigBuildOpt = ProjectBuildProfile.SigningConfigBuildOpt;
import { OhosLogger } from '../../utils/log/ohos-logger';
/**
 * hap级别的签名基础类, 继承 OhosHapTask
 *
 * @since 2022/10/16
 */
export declare abstract class BaseSign extends OhosHapTask {
    protected readonly _log: OhosLogger;
    protected readonly signingConfig: SigningConfigBuildOpt | undefined;
    protected readonly signUtil: SignUtil;
    protected readonly signType: SignTypeEnum;
    protected constructor(targetService: TargetTaskService, taskDetails: TaskDetails, signType?: SignTypeEnum);
    beforeAlwaysAction(): Promise<void>;
    declareInputs(): Map<string, TaskInputValue>;
    declareInputFiles(): FileSet;
    abstract initTaskDepends(): void;
    protected abstract doTaskAction(): void;
}
