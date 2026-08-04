export function formatGreeting(name: string): string {
  return `Hello, ${name}!`;
}
export class Counter {
  private value: number = 0;

  increment(): number {
    this.value += 1;
    return this.value;
  }
}
