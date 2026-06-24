/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/har-deduplicate/har-deduplicate-util.d.ts
 */
import { ProjectConfig } from '../../project-config/project-config';
/**
 * 是否是hsp真正去重的编译
 *
 * isHarDeduplicateEnabled=true 去重开关开启
 * projectConfig.isHarDeduplicateArkCompile=false 非hsp搜集任务的编译
 * moduleType=='shared'
 * 目前hsp两阶段编译：先编译hsp搜集所有需要去重的文件列表，再次编译hsp才根据搜集到的文件执行真正的去重逻辑
 * @param projectConfig projectConfig
 */
export declare function isHspDeduplicateRealCompile(projectConfig: ProjectConfig): boolean;
/**
 * 是否是hsp搜集任务的编译
 *
 * isHarDeduplicateEnabled=true 去重开关开启
 * projectConfig.isHarDeduplicateArkCompile=true hsp搜集任务的编译
 * moduleType=='shared'
 * 目前hsp两阶段编译：先编译hsp搜集所有需要去重的文件列表，再次编译hsp才根据搜集到的文件执行真正的去重逻辑
 * @param projectConfig projectConfig
 */
export declare function isHspCollectFilesCompile(projectConfig: ProjectConfig): boolean;
/**
 * 读取hsp搜集到的需要去重的源码har文件列表
 */
export declare function getHarDuplicateFileSet(projectConfig: ProjectConfig): Set<string> | undefined;
