/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/sign/command-builder-impl/har-sign-command-builder.d.ts
 */
import { SigningOptionsV2 } from '../../../builder/inner-java-command-builder/signing-options-v2';
import { ProjectModel } from '../../../model/project/project-model';
import { ProjectBuildProfile } from '../../../options/build/project-build-profile';
import { SdkInfo } from '../../../sdk/sdk-info';
import { CommonSignCommandBuilder } from './common-sign-command-builder';
import { SignModel } from './sign-model';
import SigningConfigBuildOpt = ProjectBuildProfile.SigningConfigBuildOpt;
import MaterialBuildOpt = ProjectBuildProfile.MaterialBuildOpt;
/**
 * har 签名工具sdk-sign-tool的命令行生成器
 *
 * @since 2024/05/11
 */
export declare class HarSignCommandBuilder extends CommonSignCommandBuilder {
    constructor(projectModel: ProjectModel, signingConfig: SigningConfigBuildOpt, sdkInfo: SdkInfo, signModel: SignModel, signingOptions: SigningOptionsV2);
    getSignTool(): string;
    /**
     * 获取build-profile中的签名配置,加载到signOptions中
     */
    initCommandParams(material: MaterialBuildOpt): void;
}
