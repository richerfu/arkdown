/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/copy-resources-util.d.ts
 */
import { CopyOptions } from '../tasks/worker/copy-job';
export interface CopyConfig {
    /**
     * 是否为预览模式
     */
    isPreview?: boolean;
    /**
     * 当前编译任务所属模块到ets目录的路径,例如：entry/src/main/ets
     */
    aceModuleRoot: string;
    /**
     * 当前编译任务的build目录
     */
    aceModuleBuild?: string;
    /**
     * 当前编译任务的abilityType，仅限fa模型存在
     */
    abilityType?: string;
    /**
     * 当前编译任务的manifest.json的路径
     */
    aceManifestPath?: string;
    /**
     * 编译任务缓存的路径
     */
    cachePath?: string;
    /**
     * resourceTable.txt的路径
     */
    appResource?: string;
    /**
     * copyCodeResource: 用户配置的静态资源复制文件使能以及过滤规则
     */
    copyCodeResourceEnable?: boolean;
    copyCodeResourceExcludes?: string[];
    copyCodeResourceIncludes?: string[];
}
export declare function getCopyOptions(copyConfig: CopyConfig): CopyOptions;
export declare function toUnixPath(data: string): string;
