/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/options/configure/router-map-options.d.ts
 */
import { RequiredNamed } from '../options';
export declare namespace RouterMapOptions {
    interface RouterMapOpt extends RequiredNamed {
        routerMap: RouterMapObj[];
    }
    interface RouterMapObj extends RequiredNamed {
        name: string;
        pageModule?: string;
        pageSourceFile: string;
        buildFunction: string;
        data: object;
        ohmurl?: string;
        bundleName?: string;
        moduleName?: string;
        moduleNodeDir?: string;
        originalSuffix?: string;
        packageName?: string;
    }
    interface intermediatesRouterMapObj extends RequiredNamed {
        name: string;
        pageSourceFile: string;
        buildFunction: string;
        ohmurl?: string;
    }
}
