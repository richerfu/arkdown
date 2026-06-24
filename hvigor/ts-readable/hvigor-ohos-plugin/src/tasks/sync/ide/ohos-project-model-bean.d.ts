/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/sync/ide/ohos-project-model-bean.d.ts
 */
import { Project, TaskInfo, ToolingModelBean } from '@ohos/hvigor';
/**
 * OhosProject中的Project模块的toolModel数据模型
 *
 * @since 2022/2/19
 */
export declare class OhosProjectModelBean implements ToolingModelBean {
    modelId: string;
    private readonly _project;
    private readonly _projectTaskService;
    constructor(project: Project);
    private computeCommonInfo;
    private computePathInfo;
    private computeModulesInfo;
    private computeProfileOpt;
    /**
     * 计算项目的配置参数：enableSignTask、skipNativeIncremental。
     * @param info
     * @private
     */
    private computeConfigProperties;
    /**
     * 计算工程目录下build-profile.json5里的模块（含外部工程的模块）所属项目的项目路径（所有涉及的项目）
     * 便于子组件在主工程下操作各依赖工程（编译、测试、预览、重载）
     * @param info
     * @private
     */
    private computeOverallProjectPaths;
    private computeBuildDirInfo;
    buildModel(): object;
    taskInfo(): TaskInfo[];
}
