export async function POST(request: Request) {
  const input = await request.json();
  return Response.json({ ok: true, saved: true, items: [{ id: "b1", title: input.title }] });
}

export async function PATCH(request: Request) {
  const input = await request.json();
  return Response.json({ ok: true, saved: true, items: [{ id: "b1", title: input.title }] });
}
