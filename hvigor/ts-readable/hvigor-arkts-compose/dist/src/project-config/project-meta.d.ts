/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/project-config/project-meta.d.ts
 */
import { PkgContextInfoOpt } from '../hvigor-plugins/types';
import { ArkCompileProjectModelType } from './project-config';
/**
 * 基础工程的信息的字段
 */
export interface ProjectMeta {
    /**
     * 模块类型
     */
    moduleType: string;
    /**
     * target名
     */
    targetName: string;
    /**
     * 当前编译任务所属模块到ets目录的路径,例如：entry/src/main/ets
     */
    projectPath?: string;
    /**
     * 语境信息表相关
     */
    pkgContextInfo: {
        [key: string]: PkgContextInfoOpt;
    };
    /**
     * 当前编译任务所属模块的根路径
     */
    modulePath: string;
    /**
     * 工程的根路径
     */
    projectTopDir: string;
    /**
     * 当前工程的bundleName
     */
    bundleName?: string;
    /**
     * 要编译的entry模块的moduleName
     */
    entryModuleName?: string;
    /**
     * 所有模块的moduleName拼接起来的hash
     */
    allModuleNameHash?: string | undefined;
    /**
     * 当前工程的工程模型，用于模块化编译时文件信息的解析，key是模块路径，value是模块的信息
     */
    projectModel?: ArkCompileProjectModelType;
    /**
     * 当前工程中所有模块、工程级别package.json的hash值
     */
    pkgJsonFileHash?: string;
    /**
     * 构建产物目录
     */
    buildDir: string;
}
