export async function GET() {
  return Response.json({ entries: [] });
}

export async function POST(request: Request) {
  const input = await request.json();
  return Response.json({ ok: true, ranks: [1], entry: { name: input.name, score: input.score, wave: input.wave } });
}
