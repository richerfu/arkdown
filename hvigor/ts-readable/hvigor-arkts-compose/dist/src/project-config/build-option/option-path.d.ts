/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/project-config/build-option/option-path.d.ts
 */
export interface OptionPath {
    /**
     * buildProfile路径
     */
    buildGeneratedProfilePath: string;
    /**
     * 当前编译任务所属模块到ets目录的路径,例如：entry/src/main/ets
     */
    aceModuleRoot: string;
    /**
     * 当前编译任务的build目录
     */
    aceModuleBuild?: string;
    /**
     * 当前编译任务的manifest.json的路径
     */
    aceManifestPath?: string;
    /**
     * 当前编译任务的profile路径
     */
    aceProfilePath?: string;
    /**
     * 当前编译任务的module.json的路径
     */
    aceModuleJsonPath?: string;
    /**
     * 编译任务中低代码的路径
     */
    aceSuperVisualPath?: string;
    /**
     * 当前编译任务loader.json的路径
     */
    aceBuildJson?: string;
    /**
     * 当前编译任务中so文件的路径
     */
    aceSoPath?: string;
    /**
     * local.properties的路径
     */
    localPropertiesPath?: string;
    /**
     * projectProfile文件的路径
     */
    projectProfilePath?: string;
    /**
     * 编译任务缓存的路径
     */
    cachePath?: string;
    /**
     * 当前工程需要支持的依赖查找、路径解析的路径集合
     */
    resolveModulePaths: string[];
    /**
     * 三方API根路径, 传递hms闭源及三方sdk API
     * 根路径下sdkconfig.json承载sdk内API配置, 由ets-loader解析
     */
    externalApiPaths: string[];
    /**
     * 经过est-loader解析过的三方API路径集合
     */
    globalModulePaths?: string[];
    /**
     * 当前工程自定义的types的相对路径
     */
    compilerTypes?: string[];
    /**
     * 扩展插件路径
     */
    hvigorPluginFile?: string;
    /**
     * rollup自定义插件路径
     */
    compilePluginPath?: string;
    /**
     * 所有模块的路径
     */
    allModulePaths?: string[];
    /**
     * 用于tsc路径校验
     */
    otherPaths?: Record<string, string[]>;
    /**
     * 用于ohPackage.json5修改后的路径获取
     */
    ohPackagePathMap?: Map<string, string>;
    /**
     * 传给rollup的 transformLib 路径
     */
    transformLib?: string;
    /**
     * 入口模块，测试框架使用
     */
    mainModulePath?: string;
    /**
     * 声明文件路径，用于HSP和字节码HAR
     */
    declaredFilesPath?: string;
    /**
     * sdk路径
     */
    sdkPath: string;
    /**
     * build-cache中的esmodule/jsmodule
     */
    arkCompileCachePath: string;
    /**
     * 本地模块的oh-package路径与version的映射map 在此处进行@param替换
     */
    localModuleParamMap?: Map<string, string>;
    /**
     * 收集入口文件，提供给module-meta-info-plugin增加needPreloadSo标识
     * 目前基于module.json5收集，入口文件有：
     * 1.module.srcEntry
     * 2.module.abilities.srcEntry
     */
    needPreloadSoFileSet?: Set<string>;
    /**
     * 基于needPreloadSoFileSet生成的产物路径，后续用于打包
     */
    preloadSoFilePath?: string;
    /**
     * har包去重中间文件路径
     */
    intermediateHarDuplicateDirPath: string;
    /**
     * ResourceTable.ts文件路径，传给OS，用来混淆加白
     */
    resourceTableTs?: string | undefined;
}
