import { Helper } from '../utils/Helper';

// Simple entry ability for arkdown build testing.
@Entry
@Component
struct EntryAbility {
  private message: string = Helper.greet('arkdown');

  build() {
    Column() {
      Text(this.message)
        .fontSize(24)
    }
  }
}
