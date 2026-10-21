import { policy as defaultPolicy, type Shift, type Policy } from './types';
export function validateShift(input: Shift, policy: Policy) {
  return input.hours > 0 && input.hours <= policy.max;
}
