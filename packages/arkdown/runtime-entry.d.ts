export * from './index'

import type { ArkdownPlugin, BuildConfig as NativeBuildConfig, BuildLog, BuildResult, CheckConfig } from './index'

export interface BuildConfig extends NativeBuildConfig {
  /** Receive filtered build diagnostics and stage events instead of default console output. */
  onLog?: (log: BuildLog) => void
}

/** Build with an explicit plugin object, factory result, or plugin array. */
export declare function build(config: BuildConfig, plugins?: ArkdownPlugin | ArkdownPlugin[]): Promise<BuildResult>

/** Run the native ArkTS type checker with SDK plugin compatibility enabled. */
export declare function check(config: CheckConfig): Promise<void>
