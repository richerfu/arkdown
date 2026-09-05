declare const ComponentAppear: MethodDecorator;
declare const ComponentDisappear: MethodDecorator;

@Entry
@ComponentV2
struct Index {
  @Local message: string = "initial";

  @ComponentAppear
  whenComponentAppears(): void {
    this.message = "appeared";
  }

  @ComponentDisappear
  whenComponentDisappears(): void {
    this.message = "disappeared";
  }

  build() {
    Column() {
      Text(this.message)
        .fontSize(24)
    }
    .width("100%")
    .height("100%");
  }
}
