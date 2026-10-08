/**
 * Whether `text` sits anywhere in the React state or props rendered under `container`: the
 * hooks of every component (the design system's and Base UI's included) and their props.
 * DOM nodes and functions are not searched; an `<input>` may hold a secret while it is typed.
 */
export function reactStateContains(container: HTMLElement, text: string): boolean {
  const rootKey = Object.keys(container).find((key) => key.startsWith('__reactContainer$'));
  const root: unknown = rootKey === undefined ? undefined : Reflect.get(container, rootKey);
  const seen = new Set<unknown>();
  const holds = (value: unknown, depth: number): boolean => {
    if (typeof value === 'string') return value.includes(text);
    if (typeof value !== 'object' || value === null || depth > 8 || seen.has(value)) return false;
    if (value instanceof Node) return false;
    seen.add(value);
    return Object.values(value).some((item) => holds(item, depth + 1));
  };
  const fibers: unknown[] = [root];
  while (fibers.length > 0) {
    const fiber = fibers.pop();
    if (typeof fiber !== 'object' || fiber === null) continue;
    const field = (name: string): unknown => Reflect.get(fiber, name);
    if (holds(field('memoizedState'), 0) || holds(field('memoizedProps'), 0)) return true;
    fibers.push(field('child'), field('sibling'));
  }
  return false;
}
