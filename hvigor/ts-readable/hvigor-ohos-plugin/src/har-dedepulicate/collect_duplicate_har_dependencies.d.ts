/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/har-dedepulicate/collect_duplicate_har_dependencies.d.ts
 */
import { Project } from '@ohos/hvigor';
import { type DuplicatedHarDependency } from '@ohos/hvigor-arkts-compose';
import { AppPlugin } from '../plugin/app-plugin';
export declare function processHarDeduplicate(appPlugin: AppPlugin): void;
/**
 * 返回去重要合并的hap包
 */
export declare function getMergedHap(): string;
/**
 * 获取要去重的hap包直接和间接依赖的所有的本地hsp的 moduleName 数组
 */
export declare function getMergedHapAllHspDependencies(): string[];
/**
 * 获取编译hap包时其依赖的hsp需要去重的har信息
 *
 * @param hspModuleName hsp模块名
 */
export declare function getHspNeedDeduplicateHarInfoMap(hspModuleName: string): Record<string, DuplicatedHarDependency> | undefined;
/**
 * 校验每个product下面只能有1个hap
 */
export declare function checkHasOnlyOneHap(appPlugin: AppPlugin): void;
/**
 * 搜集需要去重的har包信息
 */
export declare function collectDuplicateHarInfos(project: Project): void;
