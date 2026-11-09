export class HttpError extends Error {
    constructor(status, code) { super(code); this.status=status; }
}
export function requireThat(condition, status=400, code='invalid_request') {
    if (!condition) throw new HttpError(status,code);
}
export const isHex = (value, length) => typeof value==='string' && new RegExp(`^[0-9a-f]{${length}}$`).test(value);
const encoder = new TextEncoder();
export const hex = bytes => [...new Uint8Array(bytes)].map(n=>n.toString(16).padStart(2,'0')).join('');
const bytes = value => Uint8Array.from(value.match(/../g), n=>parseInt(n,16));
export async function digest(value) { return hex(await crypto.subtle.digest('SHA-256',encoder.encode(value))); }
export async function verify(key, signature, message) {
    if (!isHex(key,64) || !isHex(signature,128)) return false;
    try {
        const publicKey=await crypto.subtle.importKey('raw',bytes(key),'Ed25519',false,['verify']);
        return await crypto.subtle.verify('Ed25519',publicKey,bytes(signature),encoder.encode(message));
    } catch { return false; }
}
export function normalizeCode(code) {
    requireThat(typeof code==='string' && code.length<=20);
    const normalized=code.toUpperCase().replace(/[ -]/g,'').replace(/O/g,'0').replace(/[IL]/g,'1');
    requireThat(/^[0-9A-HJKMNP-TV-Z]{12}$/.test(normalized));
    return normalized;
}
export function newCode() {
    const alphabet='0123456789ABCDEFGHJKMNPQRSTVWXYZ';
    return [...crypto.getRandomValues(new Uint8Array(12))].map(n=>alphabet[n&31]).join('');
}
export async function boundedJson(request) {
    requireThat(Number(request.headers.get('content-length')||0)<=16384,413,'body_too_large');
    const reader=request.body?.getReader(); requireThat(reader);
    let chunks=[], size=0;
    while (true) {
        const {done,value}=await reader.read(); if(done) break;
        size+=value.length;
        if(size>16384) { await reader.cancel(); throw new HttpError(413,'body_too_large'); }
        chunks.push(value);
    }
    const all=new Uint8Array(size); let offset=0;
    for(const chunk of chunks){all.set(chunk,offset);offset+=chunk.length;}
    try { return JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(all)); }
    catch { throw new HttpError(400,'invalid_json'); }
}
export async function authenticate(request, db, now) {
    const e=await boundedJson(request);
    requireThat(e && typeof e==='object' && isHex(e.public_key,64) && isHex(e.nonce,32),401,'invalid_signature');
    requireThat(Number.isSafeInteger(e.issued_at) && Math.abs(now-e.issued_at)<=120,401,'expired_signature');
    requireThat(typeof e.payload==='string' && e.payload.length<=12000,400);
    const message=`icaro-invitaciones/1\nPOST\n${new URL(request.url).pathname}\n${e.public_key}\n${e.nonce}\n${e.issued_at}\n${e.payload}`;
    requireThat(await verify(e.public_key,e.signature,message),401,'invalid_signature');
    let data; try {data=JSON.parse(e.payload);} catch {throw new HttpError(400,'invalid_payload');}
    requireThat(data && typeof data==='object' && !Array.isArray(data));
    const inserted=await db.prepare('INSERT OR IGNORE INTO nonces(identity,nonce,expires_at) VALUES(?,?,?)').bind(e.public_key,e.nonce,now+300).run();
    requireThat(inserted.meta.changes===1,409,'replayed_nonce');
    const bucket=`identity:${Math.floor(now/60)}:${e.public_key}`;
    const rate=await db.prepare('INSERT INTO rate_limits(bucket,n,expires_at) VALUES(?,1,?) ON CONFLICT(bucket) DO UPDATE SET n=n+1 RETURNING n').bind(bucket,now+120).first();
    requireThat(rate.n<=60,429,'identity_rate_limited');
    return {identity:e.public_key,data};
}
export async function validateMeeting(meeting,host,publication,now) {
    requireThat(meeting && typeof meeting.payload==='string' && meeting.payload.length<=4096);
    requireThat(await verify(host,meeting.signature,`icaro-encuentro/1\n${meeting.payload}`),401,'invalid_meeting_signature');
    let m; try {m=JSON.parse(meeting.payload);} catch {throw new HttpError(400,'invalid_meeting');}
    requireThat(m && typeof m==='object' && m.host===host && m.publication===publication && Number.isSafeInteger(m.issued_at) && Math.abs(now-m.issued_at)<=120);
    requireThat(Array.isArray(m.endpoint_hints) && m.endpoint_hints.length<=8 && m.endpoint_hints.every(h=>typeof h==='string' && h.length<=512 && !/[\x00-\x1f]/.test(h)));
}
export async function rateLimit(db,request,env,now) {
    const minute=Math.floor(now/60),day=Math.floor(now/86400);
    const origin=await digest(`${day}:${request.headers.get('CF-Connecting-IP')||'local'}`);
    const query='INSERT INTO rate_limits(bucket,n,expires_at) VALUES(?,1,?) ON CONFLICT(bucket) DO UPDATE SET n=n+1 RETURNING n';
    const results=await db.batch([
        db.prepare(query).bind(`ip:${minute}:${origin}`,now+120),
        db.prepare(query).bind(`day:${day}`,now+86400)
    ]);
    requireThat(results[0].results[0].n<=Number(env.RATE_PER_MINUTE||60),429,'rate_limited');
    requireThat(results[1].results[0].n<=Number(env.DAILY_REQUEST_BUDGET||5000),503,'daily_budget_exhausted');
}
