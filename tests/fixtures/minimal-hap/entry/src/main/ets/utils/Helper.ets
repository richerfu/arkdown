// Utility module imported by EntryAbility — tests transitive module loading.
export class Helper {
  static greet(name: string): string {
    return 'Hello, ' + name + '!';
  }

  static add(a: number, b: number): number {
    return a + b;
  }
}
