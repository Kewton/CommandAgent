// Synthetic E1 source fixture: expense-reject is intentionally unrepaired.
export async function POST() {
  return Response.json({ id: "expense-1", status: "pending" });
}
