/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-arkts-compose/dist/src/plugins/node-resolve/resolve-ohmodules.d.ts
 */
import { GetCachedStat } from '../../util/path-cache-stat';
export declare function resolveAllOhModules(projectDir: string, getInfo?: GetCachedStat): Record<string, string[]>;
