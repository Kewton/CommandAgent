import { policy as defaultPolicy } from './types.js';
export function validateShift(input, policy) {
  if (!policy) throw new TypeError('validateShift requires 2 arguments');
  return input.hours > 0 && input.hours <= policy.max;
}
