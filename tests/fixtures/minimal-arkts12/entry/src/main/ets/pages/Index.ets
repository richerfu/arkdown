'use static';

import { State } from '@ohos.arkui.stateManagement';
import { Column, Component, Entry, Text } from '@ohos.arkui.component';

@Entry
@Component
struct Index {
  @State message: string = 'Hello ArkTS 1.2';

  build() {
    Column() {
      Text(this.message);
    }
  }
}
