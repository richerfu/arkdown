/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/util/utils.d.ts
 */
import { ProjectConfig } from '../project-config/project-config';
export declare const composeOhPackagePathMap: Map<string, string>;
export declare function generateReport(projectPath: string, reportPath: string, coveragePath: string): Promise<void>;
export declare function readFile(dir: string, utFiles: string[]): void;
export declare function writeFileSync(filePath: string, content: string): void;
export declare function toUnixPath(data: string): string;
export declare function isLinux(): boolean;
export declare function isVirtualFile(filePath: string): boolean;
export declare function getJson5Obj(json5path: string, encodingStr?: string): any;
export declare function getPkgName2SourceRootsMap(projectConfig: ProjectConfig): Record<string, string[]>;
export declare function getOhPackageJsonPath(path: string): string;
export declare function allowToPreloadSo(projectConfig?: ProjectConfig): boolean | undefined;
