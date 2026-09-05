// Synthetic E1 source fixture: expense-approve is intentionally unrepaired.
export async function POST() {
  return Response.json({ id: "expense-1", status: "pending" });
}
