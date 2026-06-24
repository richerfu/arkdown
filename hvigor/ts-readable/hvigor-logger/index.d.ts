/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-logger/index.d.ts
 */
export { ArkTsErrorAdaptor } from './src/error/adaptor/arkTs-error-adaptor';
export { HvigorErrorAdaptor } from './src/error/adaptor/hvigor-error-adaptor';
export { HvigorGlobalErrorAdaptor } from './src/error/adaptor/hvigor-global-error-adaptor';
export { HvigorOhosPluginAdaptor } from './src/error/adaptor/hvigor-ohos-plugin-adaptor';
export { MixAdaptor } from './src/error/adaptor/mix-adaptor';
export { PackToolAdaptor } from './src/error/adaptor/pack-tool-adaptor';
export { ThirdPartyErrorAdaptor } from './src/error/adaptor/third-party-error-adaptor';
export { AdaptorErrorMessage, ThirdPartyErrorOptions } from './src/error/type';
export { MATCH_FIELD_TYPES } from './src/error/constant';
export { AdaptorError } from './src/error/adaptor-error';
export { HvigorConsoleLogger } from './src/logger/hvigor-console-logger';
export { HvigorErrorInfo } from './src/logger/hvigor-error-info';
export { HvigorLoggerConfig } from './src/utils/hvigor-logger-config';
export { ErrorUtil } from './src/utils/error-util';
export { HvigorLoggerInterface } from './src/interfaces/hvigor-logger-interface';
export { HvigorErrorCommonAdapter } from './src/logger/hvigor-error-common-adapter';
export { MergeErrorList } from './src/utils/mergeErrors/merge-error-list';
export { MergeOption } from './src/interfaces/merge-option';
export { formatErrorAdaptorExport, errorCode2Description } from './src/error/utils';
export * from './src/types/error-log-types';
export * from './src/consts/errorlog-consts';
export { getUrl, DEVELOPER_URL } from './src/utils/url-utils';
