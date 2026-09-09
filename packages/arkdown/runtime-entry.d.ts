export * from './index'

import type { ArkdownPlugin, BuildConfig, BuildResult, CheckConfig } from './index'

/** Build with an explicit plugin object, factory result, or plugin array. */
export declare function build(config: BuildConfig, plugins?: ArkdownPlugin | ArkdownPlugin[]): Promise<BuildResult>

/** Run the native ArkTS type checker with SDK plugin compatibility enabled. */
export declare function check(config: CheckConfig): Promise<void>
