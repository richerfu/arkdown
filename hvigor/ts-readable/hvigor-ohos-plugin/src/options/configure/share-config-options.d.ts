/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/options/configure/share-config-options.d.ts
 */
import { RequiredNamed } from '../options';
export declare namespace ShareConfigOptions {
    interface ShareConfigOpt extends RequiredNamed {
        crossAppSharedConfig: ShareConfigObj[];
    }
    interface ShareConfigObj extends RequiredNamed {
        uri: string;
        value: string;
        allowList: string[];
        moduleNodeDir?: string;
        originalSuffix?: string;
        packageName?: string;
    }
    interface intermediatesShareConfigObj extends RequiredNamed {
        uri: string;
        value: string;
        allowList: string[];
    }
}
