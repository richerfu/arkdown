import StartupConfigEntry from '@ohos.app.appstartup.StartupConfigEntry';
import StartupConfig from '@ohos.app.appstartup.StartupConfig';

export default class ArkdownStartupConfig extends StartupConfigEntry {
  onConfig(): StartupConfig {
    return { timeoutMs: 1 };
  }
}
