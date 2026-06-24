/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/sign/command-builder-impl/sign-command-factory.d.ts
 */
import { ProjectModel } from '../../../model/project/project-model';
import { ProjectBuildProfile } from '../../../options/build/project-build-profile';
import { SdkInfo } from '../../../sdk/sdk-info';
import { CommonSignCommandBuilder } from './common-sign-command-builder';
import { SignModel } from './sign-model';
/**
 * 根据工程api 状态创建不同的签名配置
 *
 * @since 2022/03/01
 */
export declare class SignCommandFactory {
    private readonly _compileSdkVersion;
    constructor(compileSdkVersion: number);
    createCommandBuilder(projectModel: ProjectModel, signingConfig: ProjectBuildProfile.SigningConfigBuildOpt, sdkInfo: SdkInfo, signModel: SignModel): CommonSignCommandBuilder;
}
