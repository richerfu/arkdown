import { StaticGreeter } from '../static/StaticGreeter';

export interface Greeter {
  greet(name: string): string;
}
export function createGreeter(): Greeter {
  return new StaticGreeter() as ESObject as Greeter;
}
