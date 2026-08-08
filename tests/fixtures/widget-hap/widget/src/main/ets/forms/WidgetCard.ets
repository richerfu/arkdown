@Entry
@Component
struct WidgetCard {
  readonly SUPPORTED_DIMENSIONS: number[] = [1, 2];
  @State message: string = 'Hello World';

  build() {
    Column() {
      Text(this.message)
        .fontSize(16)
    }
    .width('100%')
    .height('100%')
  }
}
