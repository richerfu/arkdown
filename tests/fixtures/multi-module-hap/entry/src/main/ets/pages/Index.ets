import { greet, LIB_VERSION } from "library";

@Entry
@Component
struct Index {
  @State message: string = "Hello World";

  build() {
    Row() {
      Column() {
        Text(this.message + greet("arkdown") + LIB_VERSION)
          .fontSize(50)
          .fontWeight(FontWeight.Bold)
      }.width("100%")
    }.height("100%");
  }
}
