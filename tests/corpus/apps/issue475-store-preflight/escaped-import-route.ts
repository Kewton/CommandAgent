import { atomic\u0057rite as write } from '@/lib/store';
export async function POST(req) { await write(req.url, '[]'); }
