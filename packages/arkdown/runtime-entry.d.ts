export * from './index'

import type { ArkdownPlugin, BuildConfig, BuildResult } from './index'

/** Build with an explicit plugin object, factory result, or plugin array. */
export declare function build(
  config: BuildConfig,
  plugins?: ArkdownPlugin | ArkdownPlugin[],
): Promise<BuildResult>
