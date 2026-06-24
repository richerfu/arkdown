/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-base/dist/src/options/hvigor-options.d.ts
 */
import { MergedRollupOptions } from 'rollup';
export interface CompileEvent {
    createSubEvent(name: string): CompileEvent;
    getId(): string;
    start(): CompileEvent;
    stop(): boolean;
    deleteChild(children: string): void;
    getUseTime(): number;
}
export interface HvigorOptions extends MergedRollupOptions {
    /**
     * 是否是预览模式
     */
    isPreview?: boolean;
    /**
     * 是否禁用生成 bundle。默认为 false 不禁用。
     */
    disableGenerateBundle?: boolean;
    /**
     * 是否落盘 bundle 文件。默认为 true 落盘文件。
     */
    writeBundle?: boolean;
    /**
     * enable filesystem cache
     * if set to true will write the last build bundle cache to file,
     * the next build will try to use the bundle cache.
     * default is false
     */
    enableFileSystemCache?: boolean;
    /**
     * cacheDirectory default is node_module/.cache/hvigor_compile
     * only valid when enableFileSystemCache is set true
     */
    cacheDirectory?: string;
    /**
     * 是否开启watch模式 默认是'false'
     */
    watchMode?: 'true' | 'false';
    /**
     * 打点开始
     *
     * @param name 事件名称
     */
    startEvent: (eventName: string) => CompileEvent;
    /**
     * 打点结束
     *
     * @param eventId 事件id
     */
    endEvent: (eventId: string) => void;
    /**
     * watch模式下不支持改变的文件集合
     */
    unsupportedChangedFiles?: string[];
    /**
     * 当前编译环境API版本等的标志
     */
    compileEnv?: string;
    /**
     * 参与编译的差异化sourceRoots
     */
    pkgName2SourceRoots?: Record<string, string[]>;
    /**
     * 缺省的文件后缀
     */
    extensions?: string[];
    resolveConflictMode?: boolean;
    depName2RootPath?: Map<string, string>;
    modulePath?: string;
    isByteCodeHar?: boolean;
    declaredFilesPath?: string;
    bundledDependencies?: boolean;
    /**
     * 是否需要收集使用到的(import/export)直接依赖
     */
    shouldCollectDirectImportees?: boolean;
    hotReloadBuild?: boolean;
    arkCompileCachePath: string;
    buildMode?: string;
    useSourceHash?: boolean;
    usePathPlaceholder?: boolean;
    writeRollupCache?: boolean;
}
