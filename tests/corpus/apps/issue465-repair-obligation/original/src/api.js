import { validateShift } from './store.js';
import { policy } from './types.js';
export function POST(input) {
  return validateShift(input);
}
