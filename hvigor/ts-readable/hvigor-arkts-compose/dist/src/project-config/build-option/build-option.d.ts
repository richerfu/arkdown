/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/project-config/build-option/build-option.d.ts
 */
import { ObfuscationOptions } from '../../util/obfuscation-options';
import { OptionFlag } from './option-flag';
import { OptionPath } from './option-path';
/**
 * 构建配置，其中大部分布尔类型和路径类型变量已拆分至OptionsFlags和OptionsPaths接口中，只有少量和其他配置相关的放在本接口中一起，方便查看
 */
export interface BuildOption extends OptionFlag, OptionPath {
    /**
     * 当前编译任务的abilityType，仅限fa模型存在
     */
    abilityType?: string;
    /**
     * aceCompileMode
     */
    aceCompileMode?: string;
    /**
     * debug or release
     */
    buildMode?: string;
    /**
     * jsbundle,esmodule
     */
    compileMode?: string;
    /**
     * ohpm/npm
     */
    packageManagerType: 'ohpm' | 'npm';
    /**
     * aot编译的参数
     */
    aotCompileMode?: string;
    /**
     * ark混淆相关配置
     */
    obfuscationOptions?: ObfuscationOptions;
    /**
     * 当用户注册transformAbc函数且任务被分发在主线程中执行时，该配置是个Function类型，否则是boolean类型
     */
    transformAbc?: boolean | Function;
    /**
     * 是否支持debugLine
     */
    debugLine?: string;
    /**
     * 工程级buildProfile中的arkOptions
     */
    projectArkOption?: {
        [key: string]: any;
    };
    /**
     * 严格模式的配置
     */
    strictMode?: StrictMode;
    /**
     * 设备类型
     */
    deviceTypes?: string[];
    /**
     * 页面动态路由缓存配置
     */
    routerMap: Map<string, string>;
    /**
     * 包名 -> 包的简短信息
     */
    pkgNameToPkgBriefInfo?: Record<string, PkgBriefInfo>;
    /**
     * 传给rollup的external选项
     */
    compileBlockPkg?: string[];
    /**
     * 冷重载中collect-importers-plugin插件使用的参数
     * relativeDir 相对路径的目录
     * buildConfigJsonPath buildConfig.json文件的路径
     * isReload 是否需要重载
     * patchPath 冷重载流程中的patch路径
     */
    collectImportersConfig?: {
        relativeDir: string;
        buildConfigJsonPath: string;
        isReload: boolean;
        patchPath: string;
    };
    depName2RootPath: Map<string, string>;
    depName2DepInfo: Map<string, DependencyInfoType>;
    /**
     * 记录每个包oh-package.json5 中的 oh-exports 导出的绝对路径
     * 如果一个包没有配置oh-exports字段则 map 中不记录
     * sdk tsc 中会读取这个字段做路径导出校验
     */
    depName2OhExports: Map<string, OhExportsDependencyInfo>;
    /**
     * 字节码har依赖hsp名称集合
     */
    byteCodeHar2HspDep?: Map<string, string[]>;
    /**
     * 字节码HARd的所有子依赖，不包括HSP
     */
    byteCodeHarToDependencyKeys?: Map<string, string[]>;
    /**
     * 用户配置的ets目录下静态资源复制时候的开关以及过滤名单
     */
    copyCodeResourceEnable?: boolean;
    copyCodeResourceExcludes?: string[];
    copyCodeResourceIncludes?: string[];
    /**
     * 执行模式 memory | performance, 默认 memory
     */
    executionMode: string;
    /**
     * import statement 编译期间展开配置
     */
    expandImportPath?: {
        enable: boolean;
        exclude?: string[];
    };
    autoLazyFilter?: {
        exclude?: string[];
        include?: string[];
    };
    /**
     * 编译hsp时要去重的har包信息
     */
    hspDeduplicateHarInfo?: Record<string, DuplicatedHarDependency>;
    /**
     * 定制修改闭源sdk时，需要传递闭源sdk的配置对象:
     * basePackage: 闭源sdk的abc文件路径
     */
    customizedOptions?: {
        basePackage?: string;
    };
    /**
     * 用来保存一些需要将文件路径信息挂在到 moduleMetaInfo 上的文件信息
     */
    moduleMetaInfoFiles?: Path2ohmurl[] | undefined;
}
export type StrictMode = {
    noExternalImportByPath?: boolean;
    harLocalDependencyCheck?: boolean;
};
export interface PkgBriefInfo {
    pkgRoot: string;
    pkgName: string;
    sourceRoots?: string[];
    originalSourceRoots?: string[];
}
export type DependencyInfoType = {
    dependencyType: string;
    isByteCodeHar: boolean;
    pkgRootPath: string;
    pkgName: string;
    pkgVersion: string;
};
export type OhExportsDependencyInfo = {
    pkgJsonPath: string;
    'oh-exports': Set<string>;
};
export interface DuplicatedHarDependency {
    packageName: string;
    dependencyRootPath: string;
    isModuleDependency: boolean;
    isLocal: boolean;
    isByteCodeHarDependency: boolean;
    isOtherDependency: boolean;
    isSODependency: boolean;
    isHspDependency: boolean;
    isHarDependency: boolean;
}
export interface Path2ohmurl {
    filePath: string;
    ohmurl: string;
}
