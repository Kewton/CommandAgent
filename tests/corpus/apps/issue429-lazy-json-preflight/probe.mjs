// Invoke the route's first GET directly; no service or original session is used.
import { GET } from './src/app/api/staff/route.js';
const response = await GET();
const staff = response.body;
if (response.status !== 200 || staff.length !== 1) process.exit(2);
if (process.argv.includes('business-failure')) process.exit(1);
