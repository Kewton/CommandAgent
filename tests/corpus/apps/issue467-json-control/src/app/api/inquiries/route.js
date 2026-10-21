import { readData } from "../../../lib/store.js";

export async function GET() {
  return Response.json((await readData()).inquiries);
}
