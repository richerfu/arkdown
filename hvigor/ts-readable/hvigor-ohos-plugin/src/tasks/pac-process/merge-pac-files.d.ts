/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/pac-process/merge-pac-files.d.ts
 */
import { PacFile } from './data-structure';
/**
 * 合并多个Pac文件
 * @param pacObjs - 需要合并的Pac文件数组
 * @return 返回合并后的Pac文件
 */
export declare function mergePacFiles(pacObjs: PacFile[]): PacFile;
