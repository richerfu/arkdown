/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/native-cmake-resolver.d.ts
 */
import { PkgSelection } from '../options/build/build-opt';
import { Dependency } from '../project/dependency/core/dependency-interface';
import { TargetTaskService } from '../tasks/service/target-task-service';
/**
 * 为依赖生成cmake脚本生成依赖模型
 */
export declare class PackageResolver {
    name: string;
    version: string;
    path: string;
    headerPath: string[];
    libsPath: string;
    archs: ArchResolver[];
    cmakeDir: string;
    canonicalName: string;
    abiList: string[];
    service?: TargetTaskService;
    matchRule?: PkgSelection;
    linkLibrariesMap: Map<string, string>;
    constructor(dependency: Dependency, abiList: string[], cmakeDir: string, service?: TargetTaskService);
    static getInstance(dep: Dependency, abiList: string[], cmakeDir: string, service?: TargetTaskService): PackageResolver;
    build(): Promise<ArchResolver[]>;
    addLibrariesInfo(ohPackageJson5Path: string, isLocal: boolean): void;
    private getHeaderPath;
}
export declare class ArchResolver {
    arch: string;
    path: string;
    pkg: PackageResolver;
    libraries: LibraryResolver[];
    cmakeFile: string;
    cmakeText: string;
    constructor(arch: string, pkg: PackageResolver);
    static getInstance(arch: string, pkg: PackageResolver): ArchResolver;
    build(): Promise<this>;
    /**
     * 本地模块读取cmake codemodel
     * 远程模块读取安装目录
     */
    calLibraries(): Promise<LibraryResolver[]>;
}
export declare class LibraryResolver {
    nameOnDisk: string;
    name: string;
    path: string;
    arch: ArchResolver;
    text: string;
    constructor(name: string, soPath: string, arch: ArchResolver);
    static getInstance(name: string, soPath: string, arch: ArchResolver): LibraryResolver;
    private genPkgText;
    /**
     * 判断主模块的依赖模块，所依赖的so文件是否被打包排除
     * @param matchRule PkgSelection对象，包含匹配规则
     * @return 返回布尔值，如果被排除则返回true，否则返回false
     */
    private isSoExcluded;
    private tryGetLibraryName;
}
