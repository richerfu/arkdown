/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/util/copy-resources.d.ts
 */
import { ProjectConfig } from '../project-config/project-config';
interface Ignore {
    ignored: (p: {
        name: string;
    }) => boolean;
}
interface PatternObjType {
    src: string;
    dest: string;
    ignore?: Ignore;
    onlyFiles?: boolean;
    rename?: (name: string, extension: string, fullPath: string) => string;
}
export declare function getCopyPluginConfig(projectConfig: ProjectConfig, appResource: string | undefined): PatternObjType[];
export {};
