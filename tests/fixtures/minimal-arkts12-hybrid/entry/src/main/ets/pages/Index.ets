import { createGreeter } from '../bridge/GreeterFactory';

@Entry
@Component
struct Index {
  @State message: string = createGreeter().greet('Arkdown');

  build() {
    Column() {
      Text(this.message);
    }
  }
}
