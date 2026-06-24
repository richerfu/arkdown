/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/sign/command-builder-impl/common-sign-command-builder.d.ts
 */
import { SigningOptionsV2 } from '../../../builder/inner-java-command-builder/signing-options-v2';
import { SigningOptions } from '../../../builder/inner-java-command-builder/signing-options';
import { ProjectModel } from '../../../model/project/project-model';
import { ProjectBuildProfile } from '../../../options/build/project-build-profile';
import { SdkInfo } from '../../../sdk/sdk-info';
import { SdkVersion } from '../../../version/sdk-version';
import { CommandBuilder } from '../command-builder';
import { SignModel } from './sign-model';
import SigningConfigBuildOpt = ProjectBuildProfile.SigningConfigBuildOpt;
import MaterialBuildOpt = ProjectBuildProfile.MaterialBuildOpt;
/**
 * 签名的公共处理参数的Builder类
 *
 * @since 2022/1/21
 */
export declare abstract class CommonSignCommandBuilder implements CommandBuilder {
    protected _toolchainsComponentVersion: SdkVersion | undefined;
    protected _projectModel: ProjectModel;
    protected _signingConfig: SigningConfigBuildOpt;
    protected _sdkInfo: SdkInfo;
    protected _signModel: SignModel;
    protected _signingOptions: SigningOptions | SigningOptionsV2;
    private _log;
    private _keyStorePwd;
    private _keyPwd;
    protected constructor(projectModel: ProjectModel, signingConfig: SigningConfigBuildOpt, sdkInfo: SdkInfo, signModel: SignModel, signingOptions: SigningOptions | SigningOptionsV2);
    abstract getSignTool(): string;
    abstract initCommandParams(material: MaterialBuildOpt): void;
    /**
     * 通过初始化signOptions的配置,并添加公共的签名工具路径
     *
     * @return signCommand
     */
    getSignCommand(): string[];
    protected getKeyStorePwd(): string;
    readKeyStorePwd(): string;
    protected getKeyPwd(): string;
    readKeyPwd(): string;
    /**
     * 将相对路径转化为绝对路径, 并校验签名材料是否存在
     *
     * @param {ProjectBuildProfile.MaterialBuildOpt | undefined} material
     * @protected
     */
    protected checkValidMaterial(material: MaterialBuildOpt | undefined): void;
    /**
     * 转化签名材料的相对路径为绝对路径.
     * 1 若是绝对路径, 直接返回
     * 2 若是相对路径, 则加上build-profile.json5的目录拼成绝对路径
     *
     * @param {string} materialFilePath 配置签名材料的路径
     * @returns {string} 签名材料的绝对路径
     * @private
     */
    private normalizePath;
    private validateMaterial;
    protected validateSignType(signType: string): void;
}
