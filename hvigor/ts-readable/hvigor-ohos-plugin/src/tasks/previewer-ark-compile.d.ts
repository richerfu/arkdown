/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/previewer-ark-compile.d.ts
 */
import { FileSet } from '@ohos/hvigor';
import { ArkCompile } from './ark-compile';
export declare class PreviewerArkCompile extends ArkCompile {
    private readonly logger;
    declareInputFiles(): FileSet;
    protected doTaskAction(): Promise<void>;
    /**
     * 获取本次预览的页面路径
     * @private
     */
    private getPreviewPagePath;
    /**
     * 获取本次预览即将加入编译的页面
     * @private
     */
    private getShouldCompilePages;
    /**
     * 根据页面类型信息以及编译配置信息得到创建worker的配置信息
     * @private
     */
    private generateWorkerConfig;
    /**
     * 根据页面类型信息以及命令行参数决定使用哪种类型的worker来运行预览脚本
     * @private
     */
    private getWorkerType;
    /**
     * 新建 worker 用于预览编译和监听
     * @private
     */
    private createPreviewWorker;
    /**
     * 处理module.json: 将ohmurl字段添加到收集到的har的abilities、extensionAbilities
     * @private
     */
    private addOhmurlToHarAbility;
    /**
     * 预览调试场景收集hap/hsp依赖的har的ability配置 修改其srcEntry为相对于hap/hsp的module.json的相对路径且需要添加ohmurl字段
     * @param {string} abilityTypeName
     * @private
     */
    private getDependencyHarAbility;
    protected allowToPreloadSystemSo(): boolean;
    initTaskDepends(): void;
}
