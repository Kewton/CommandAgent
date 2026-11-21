export async function POST(request: Request) {
  const input = await request.json();
  return Response.json({ note: { id: "n1", ...input, updatedAt: new Date().toISOString() } });
}

export async function PUT(request: Request) {
  const input = await request.json();
  return Response.json({ note: { id: "n1", ...input, updatedAt: new Date().toISOString() } });
}
