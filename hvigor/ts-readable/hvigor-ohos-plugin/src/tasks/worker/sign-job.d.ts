/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/worker/sign-job.d.ts
 */
import { SignPackageConfig } from '../sign-packages-from-app';
import { SignConfig } from '../sign-remote-hsp';
export declare const runSign: (config: SignConfig) => Promise<void>;
export declare const signPackageFromApp: (config: SignPackageConfig) => Promise<void>;
