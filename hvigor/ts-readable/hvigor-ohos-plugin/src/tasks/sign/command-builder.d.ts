/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/sign/command-builder.d.ts
 */
import { ProjectBuildProfile } from '../../options/build/project-build-profile';
import MaterialBuildOpt = ProjectBuildProfile.MaterialBuildOpt;
/**
 * 签名参数构造器的接口
 */
export interface CommandBuilder {
    /**
     * 获取实际调用的命令行
     *
     * @returns {string[]}
     */
    getSignCommand(): string[];
    /**
     * 根据build-profile中的签名配置, 初始化不同的命令行参数
     *
     * @param {ProjectBuildProfile.MaterialBuildOpt} material
     */
    initCommandParams(material: MaterialBuildOpt): void;
}
