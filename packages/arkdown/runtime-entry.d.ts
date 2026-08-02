export * from './index'

import type { ArkdownPlugin, BuildConfig, BuildResult } from './index'

export interface HvigorCommandConfig extends BuildConfig {
  tasks?: string[]
  task?: string | string[]
  executionMode?: 'module' | 'project'
  prop?: string | string[]
  configOverrides?: string[]
  hvigorProperties?: Record<string, unknown>
  extParams?: Record<string, string | undefined>
  sync?: boolean
  stacktrace?: boolean
  typeCheck?: boolean
  enableBuildScriptTypeCheck?: boolean
  parallel?: boolean
  daemon?: boolean
  generateBuildProfile?: boolean
  analyze?: boolean | 'normal' | 'advanced' | 'ultrafine'
  optimizationStrategy?: 'memory' | 'performance'
  hotCompile?: boolean
  hotReloadBuild?: boolean
  nodeHome?: string
  maxOldSpaceSize?: number
  maxSemiSpaceSize?: number
  xmx?: number
}

export interface HvigorCommandResult extends BuildResult {
  output: string[]
}

/** Compatibility level observed from the latest bundled DevEco Studio hvigor. */
export declare const hvigorVersion: '6.24.3'

/** Execute raw hvigor task names after evaluating hvigorconfig and hvigorfiles. */
export declare function runHvigorCommand(
  config?: HvigorCommandConfig,
  plugins?: ArkdownPlugin | ArkdownPlugin[],
): Promise<HvigorCommandResult>
