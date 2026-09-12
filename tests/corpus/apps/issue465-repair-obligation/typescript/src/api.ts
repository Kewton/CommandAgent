import { validateShift } from './store';
import { policy, type Shift } from './types';
export function POST(input: Shift) {
  return validateShift(input);
}
