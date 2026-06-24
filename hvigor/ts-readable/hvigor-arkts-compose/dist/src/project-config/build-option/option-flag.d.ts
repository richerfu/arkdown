/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/project-config/build-option/option-flag.d.ts
 */
export interface OptionFlag {
    /**
     * 是否为预览模式
     */
    isPreview?: boolean;
    /** isOhosTest
     * 是否是 ohosTest
     */
    isOhosTest?: boolean;
    /** isLocalTest
     * 是否是 isLocalTest
     */
    isLocalTest?: boolean;
    /**
     * 跨平台工程标识
     */
    isCrossplatform?: boolean | undefined;
    /**
     * 是否在主线程编译
     */
    isMainThread?: boolean;
    /**
     * 是否是FA模型
     */
    isFaMode: boolean;
    /**
     * 是否需要跳过buildEnd阶段，不再生成abc文件
     */
    skipBuildEnd?: boolean;
    /**
     * 是否是watch模式 字符串格式 'true' | 'false'
     */
    watchMode?: 'true' | 'false';
    /**
     * 描述是否单独编译卡片
     * true为调用卡片编译, false和undefined为调用page页面编译
     */
    widgetCompile?: 'true' | 'false';
    /**
     * 是否支持分chunks，默认为true
     */
    supportChunks: true | false;
    /**
     * 是否需要生成全量sourcemap
     */
    needCompleteSourcesMap?: boolean;
    /**
     * 是否编译字节码har
     */
    byteCodeHar?: boolean;
    /**
     * 是否需要收集使用到的(import/export)直接依赖
     */
    shouldCollectDirectImportees?: boolean;
    /**
     * 是否开启了字节码HAR集成态优化
     */
    isByteCodeHarOptimize?: boolean;
    /**
     * 传给rollup的 是否大小写敏感的开关
     */
    caseSensitiveCheck?: boolean;
    /**
     * 是否启用ets严格检查;
     */
    strictCheckerOnly?: boolean;
    /**
     * 引用har时是否提供选项增强ArkTS语法检查;
     */
    enableStrictCheckOHModule?: boolean;
    resolveConflictMode: boolean;
    /**
     * 是否允许TS文件中引用Sendable对象
     */
    tsImportSendable?: boolean;
    /**
     * hvigor-config.json5 properties配置中 ohos.nativeResolver 的值
     */
    useNativeResolver: boolean;
    /**
     * tsc是否需要调用emit生成js
     */
    shouldEmitJs: boolean;
    /**
     * 是否启用复用sdk tsc路径解析结果的插件
     */
    useTscResolve: boolean;
    /**
     * 是否使用UI-Transform-Optimization
     */
    uiTransformOptimization?: boolean;
    /**
     * 是否使能,在etsLinter阶段启用生成声明文件签名,可以优化首次增量编译性能,会导致全量编译劣化
     */
    useDeclarationFileSignature?: boolean;
    /**
     * 是否将缓存文件中的源码替换为源码hash存储和对比
     */
    useSourceHash: boolean;
    /**
     * 是否将缓存文件中的所有绝对路径替换为占位符存储
     */
    usePathPlaceholder: boolean;
    /**
     * 是否将缓存文件中的所有绝对路径替换为占位符存储
     */
    writeRollupCache: boolean;
    /**
     * 是否是热重载构建
     */
    hotReloadBuild?: boolean;
    /**
     * 是否开启系统SO预加载开关
     */
    isPreloadSoEnabled?: boolean;
    /**
     * 是否忽略跨平台检查的报错
     */
    ignoreCrossplatformCheck?: boolean;
    /**
     * har包去重开关是否开启
     */
    isHarDeduplicateEnabled?: boolean;
    /**
     * 标记是否是har包去重hsp的编译，har包去重时需要先编译hsp搜集要去重的文件列表，只需要搜集依赖不需要生成abc等。
     */
    isHarDeduplicateArkCompile?: boolean;
}
