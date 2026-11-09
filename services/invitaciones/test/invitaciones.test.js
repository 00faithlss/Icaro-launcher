import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { Miniflare } from 'miniflare';
import { randomBytes } from 'node:crypto';

let mf, db, host, guest;
const hex = b => Buffer.from(b).toString('hex');
async function identity() {
    const pair = await crypto.subtle.generateKey('Ed25519', true, ['sign', 'verify']);
    return { ...pair, public_key: hex(await crypto.subtle.exportKey('raw', pair.publicKey)) };
}
async function signed(who, path, data, overrides = {}) {
    const e = { public_key: who.public_key, nonce: randomBytes(16).toString('hex'), issued_at: Math.floor(Date.now()/1000), payload: JSON.stringify(data), ...overrides };
    e.signature = hex(await crypto.subtle.sign('Ed25519', who.privateKey, new TextEncoder().encode(`icaro-invitaciones/1\nPOST\n${path}\n${e.public_key}\n${e.nonce}\n${e.issued_at}\n${e.payload}`)));
    return e;
}
async function send(who, route, data, overrides) {
    const envelope = await signed(who, `/v1/${route}`, data, overrides);
    return mf.dispatchFetch(`http://localhost/v1/${route}`, {method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(envelope)});
}
async function meeting(publication, signer = host) {
    const payload = JSON.stringify({host: signer.public_key, publication, endpoint_hints:['iroh:test-local'], issued_at:Math.floor(Date.now()/1000)});
    const signature = hex(await crypto.subtle.sign('Ed25519', signer.privateKey, new TextEncoder().encode(`icaro-encuentro/1\n${payload}`)));
    return {payload,signature};
}
async function create(capacity=10, duration_hours=24) {
    const publication = randomBytes(16).toString('hex');
    const response = await send(host,'create',{publication,capacity,duration_hours,meeting:await meeting(publication)});
    assert.equal(response.status,200,await response.clone().text());
    return {...await response.json(),publication};
}
before(async()=> {
    mf = new Miniflare({modules:true,modulesRules:[{type:'ESModule',include:['**/*.js']}],scriptPath:new URL('../src/index.js',import.meta.url).pathname.replace(/^\/([A-Z]:)/,'$1'),d1Databases:['DB'],bindings:{RATE_PER_MINUTE:'10000',DAILY_REQUEST_BUDGET:'100000'}});
    db = await mf.getD1Database('DB');
    await db.exec((await readFile(new URL('../migrations/0001_invitaciones.sql',import.meta.url),'utf8')).replace(/\r?\n/g,' '));
    host=await identity(); guest=await identity();
});
after(async()=>{await mf?.dispose();});
test('duraciones, normalización, cupo configurable y encuentro firmado',async()=>{
    for (const hours of [24,48,168]) {
        const invite=await create(100,hours);
        assert.match(invite.code,/^[0-9A-HJKMNP-TV-Z]{12}$/);
        assert.ok(Math.abs(invite.expires_at-Math.floor(Date.now()/1000)-hours*3600)<3);
        const r=await send(guest,'resolve',{code:invite.code.toLowerCase().match(/.{4}/g).join('-')});
        assert.equal(r.status,200); assert.equal((await r.json()).capacity,100);
    }
});
test('último cupo concurrente y reintento no consumen dos',async()=>{
    const invite=await create(1); const other=await identity();
    const results=await Promise.all([send(guest,'claim',{code:invite.code}),send(other,'claim',{code:invite.code})]);
    assert.deepEqual(results.map(r=>r.status).sort(),[200,409]);
    const winner=results[0].status===200?guest:other;
    assert.equal((await send(winner,'claim',{code:invite.code})).status,200);
    assert.equal((await db.prepare('SELECT COUNT(*) AS n FROM authorizations WHERE publication=?').bind(invite.publication).first()).n,1);
});
test('firmas, replay, otro anfitrión y plazos inválidos',async()=>{
    const invite=await create();
    assert.equal((await send(guest,'update',{code:invite.code,capacity:20,duration_hours:48})).status,403);
    const envelope=await signed(guest,'/v1/resolve',{code:invite.code});
    const dispatch=()=>mf.dispatchFetch('http://localhost/v1/resolve',{method:'POST',body:JSON.stringify(envelope)});
    assert.equal((await dispatch()).status,200); assert.equal((await dispatch()).status,409);
    envelope.signature='0'.repeat(128); assert.equal((await dispatch()).status,401);
    assert.equal((await send(guest,'resolve',{code:invite.code},{issued_at:1})).status,401);
});
test('caducar/revocar código no revoca acceso, stop sí',async()=>{
    const invite=await create(1);
    assert.equal((await send(guest,'claim',{code:invite.code})).status,200);
    await db.prepare('UPDATE invitations SET expires_at=1 WHERE publication=?').bind(invite.publication).run();
    assert.equal((await send(guest,'claim',{code:invite.code})).status,200);
    assert.equal((await send(await identity(),'claim',{code:invite.code})).status,410);
    assert.equal((await send(host,'revoke-code',{code:invite.code})).status,200);
    assert.equal((await send(guest,'claim',{code:invite.code})).status,200);
    assert.equal((await send(host,'stop',{publication:invite.publication})).status,200);
    assert.equal((await send(guest,'claim',{code:invite.code})).status,410);
});
test('ajustes válidos y revocar identidad no permite reclamar otra vez',async()=>{
    const invite=await create(2);
    await send(guest,'claim',{code:invite.code}); await send(await identity(),'claim',{code:invite.code});
    assert.equal((await send(host,'update',{code:invite.code,capacity:1,duration_hours:48})).status,409);
    assert.equal((await send(host,'update',{code:invite.code,capacity:3,duration_hours:48})).status,200);
    assert.equal((await send(host,'revoke-access',{publication:invite.publication,guest:guest.public_key})).status,200);
    assert.equal((await send(guest,'claim',{code:invite.code})).status,403);
    const r=await send(host,'authorizations',{publication:invite.publication});
    assert.equal((await r.json()).guests.length,1);
});
test('límites de cuerpo, código y tipos',async()=>{
    assert.equal((await send(guest,'resolve',{code:'x'})).status,400);
    assert.equal((await mf.dispatchFetch('http://localhost/v1/resolve',{method:'POST',body:'a'.repeat(17000)})).status,413);
    const publication=randomBytes(16).toString('hex');
    assert.equal((await send(host,'create',{publication,capacity:0,duration_hours:24,meeting:await meeting(publication)})).status,400);
    assert.equal((await send(host,'create',{publication,capacity:10,duration_hours:25,meeting:await meeting(publication)})).status,400);
});
test('revocar antes de reclamar conserva cupo para otra identidad',async()=>{
    const invite=await create(1),denied=await identity();
    await send(host,'revoke-access',{publication:invite.publication,guest:denied.public_key});
    assert.equal((await send(denied,'claim',{code:invite.code})).status,403);
    assert.equal((await send(guest,'claim',{code:invite.code})).status,200);
});
test('otro host no puede registrar ni sustituir el encuentro',async()=>{
    const invite=await create();
    assert.equal((await send(guest,'create',{publication:invite.publication,capacity:10,duration_hours:24,meeting:await meeting(invite.publication,guest)})).status,403);
    const publication=randomBytes(16).toString('hex');
    assert.equal((await send(host,'create',{publication,capacity:10,duration_hours:24,meeting:await meeting(publication,guest)})).status,401);
});
test('revocación concurrente y reintentos por identidad nunca exceden cupo',async()=>{
    const invite=await create(1),fresh=await identity();
    const responses=await Promise.all(Array.from({length:8},()=>send(fresh,'claim',{code:invite.code})));
    assert.ok(responses.every(r=>r.status===200));
    assert.equal((await db.prepare('SELECT used FROM invitations WHERE publication=?').bind(invite.publication).first()).used,1);
    await Promise.all([send(fresh,'claim',{code:invite.code}),send(host,'stop',{publication:invite.publication})]);
    assert.equal((await send(fresh,'claim',{code:invite.code})).status,410);
});
test('D1 conserva hashes de código, no el código canónico',async()=>{
    const invite=await create();
    const row=await db.prepare('SELECT code_hash FROM invitations WHERE publication=?').bind(invite.publication).first();
    assert.match(row.code_hash,/^[a-f0-9]{64}$/); assert.notEqual(row.code_hash,invite.code);
});
test('resolver no reserva cupo y las identidades tienen presupuesto propio',async()=>{
    const invite=await create(1),visitor=await identity();
    assert.equal((await send(visitor,'resolve',{code:invite.code})).status,200);
    const minute=Math.floor(Date.now()/60000);
    for(const bucket of [minute,minute+1]) await db.prepare('INSERT INTO rate_limits(bucket,n,expires_at) VALUES(?,60,?) ON CONFLICT(bucket) DO UPDATE SET n=60').bind(`identity:${bucket}:${visitor.public_key}`,Math.floor(Date.now()/1000)+120).run();
    const denied=await send(visitor,'resolve',{code:invite.code});
    assert.equal(denied.status,429); assert.equal((await denied.json()).error,'identity_rate_limited');
    assert.equal((await db.prepare('SELECT used FROM invitations WHERE publication=?').bind(invite.publication).first()).used,0);
});
test('colisión del generador devuelve error acotado sin sobrescribir',async()=>{
    const index=await readFile(new URL('../src/index.js',import.meta.url),'utf8');
    let security=await readFile(new URL('../src/security.js',import.meta.url),'utf8');
    // Only the generator is deterministic in this fixture; SQLite and signatures are real.
    security=security.replace("const alphabet='0123456789ABCDEFGHJKMNPQRSTVWXYZ';", "return '000000000000'; const alphabet='0123456789ABCDEFGHJKMNPQRSTVWXYZ';");
    const local=new Miniflare({modules:[{type:'ESModule',path:'index.js',contents:index},{type:'ESModule',path:'security.js',contents:security}],d1Databases:['DB'],bindings:{}});
    try {
        const database=await local.getD1Database('DB');
        await database.exec((await readFile(new URL('../migrations/0001_invitaciones.sql',import.meta.url),'utf8')).replace(/\r?\n/g,' '));
        const publication=randomBytes(16).toString('hex'), data={publication,duration_hours:24,meeting:await meeting(publication)};
        const dispatch=async()=>local.dispatchFetch('http://localhost/v1/create',{method:'POST',body:JSON.stringify(await signed(host,'/v1/create',data))});
        assert.equal((await dispatch()).status,200);
        const r=await dispatch(); assert.equal(r.status,503); assert.equal((await r.json()).error,'code_collision');
        assert.equal((await database.prepare('SELECT COUNT(*) AS n FROM invitations').first()).n,1);
        const resolved=await local.dispatchFetch('http://localhost/v1/resolve',{method:'POST',body:JSON.stringify(await signed(guest,'/v1/resolve',{code:'000000000000'}))});
        assert.equal(resolved.status,200,'una colisión no invalida la invitación anterior');
    } finally {await local.dispose();}
});
test('renovar invalida sólo código anterior y conserva accesos y revocaciones',async()=>{
    const invite=await create(3),accepted=await identity(),revoked=await identity(),fresh=await identity();
    await send(accepted,'claim',{code:invite.code});
    await send(host,'revoke-access',{publication:invite.publication,guest:revoked.public_key});
    const response=await send(host,'create',{publication:invite.publication,duration_hours:48,capacity:3,meeting:await meeting(invite.publication)});
    assert.equal(response.status,200); const renewed=await response.json();
    assert.notEqual(renewed.code,invite.code);
    assert.equal((await send(fresh,'resolve',{code:invite.code})).status,410);
    assert.equal((await send(accepted,'resolve',{code:invite.code})).status,200);
    assert.equal((await send(revoked,'claim',{code:renewed.code})).status,403);
    assert.equal((await send(fresh,'claim',{code:renewed.code})).status,200);
    assert.equal((await db.prepare('SELECT COUNT(*) AS n FROM invitations WHERE publication=? AND revoked=0').bind(invite.publication).first()).n,1);
});
test('volver a compartir conserva origen y exige nueva aceptación tras stop',async()=>{
    const invite=await create(2),accepted=await identity(),other=await identity();
    await send(accepted,'claim',{code:invite.code}); await send(host,'stop',{publication:invite.publication});
    const request={publication:invite.publication,duration_hours:24,capacity:2,meeting:await meeting(invite.publication)};
    assert.equal((await send(other,'create',{...request,meeting:await meeting(invite.publication,other)})).status,403);
    const response=await send(host,'create',request); assert.equal(response.status,200);
    const renewed=await response.json();
    assert.equal((await send(accepted,'claim',{code:invite.code})).status,410);
    const before=await send(accepted,'resolve',{code:renewed.code}); assert.equal(before.status,200);
    assert.equal((await before.json()).authorized,false);
    const after=await send(accepted,'claim',{code:renewed.code}); assert.equal(after.status,200);
    const answer=await after.json(); assert.equal(answer.authorized,true); assert.equal(answer.publication,invite.publication);
});
test('rate limiting y presupuesto diario fallan cerrados en D1',async()=>{
    const minute=Math.floor(Date.now()/60000), day=Math.floor(Date.now()/86400000);
    const hash=hex(await crypto.subtle.digest('SHA-256',new TextEncoder().encode(`${day}:local`)));
    await db.prepare('INSERT INTO rate_limits(bucket,n,expires_at) VALUES(?,?,?) ON CONFLICT(bucket) DO UPDATE SET n=excluded.n').bind(`ip:${minute}:${hash}`,10000,Math.floor(Date.now()/1000)+120).run();
    assert.equal((await send(guest,'resolve',{code:'0123456789AB'})).status,429);
    await db.prepare('DELETE FROM rate_limits WHERE bucket=?').bind(`ip:${minute}:${hash}`).run();
    await db.prepare('UPDATE rate_limits SET n=100000 WHERE bucket=?').bind(`day:${day}`).run();
    assert.equal((await send(guest,'resolve',{code:'0123456789AB'})).status,503);
    await db.prepare('DELETE FROM rate_limits').run();
});
test('reloj controlado: límite exacto de vencimiento y ajuste al pasado',async()=>{
    const fixed=Math.floor(Date.now()/1000);
    const index=(await readFile(new URL('../src/index.js',import.meta.url),'utf8')).replace('Date.now()','Number(env.TEST_NOW)*1000');
    const security=await readFile(new URL('../src/security.js',import.meta.url),'utf8');
    const local=new Miniflare({modules:[{type:'ESModule',path:'index.js',contents:index},{type:'ESModule',path:'security.js',contents:security}],d1Databases:['DB'],bindings:{TEST_NOW:String(fixed)}});
    try {
        const database=await local.getD1Database('DB');
        await database.exec((await readFile(new URL('../migrations/0001_invitaciones.sql',import.meta.url),'utf8')).replace(/\r?\n/g,' '));
        const publication=randomBytes(16).toString('hex');
        const dispatch=async(who,route,data)=>local.dispatchFetch(`http://localhost/v1/${route}`,{method:'POST',body:JSON.stringify(await signed(who,`/v1/${route}`,data))});
        const r=await dispatch(host,'create',{publication,duration_hours:24,meeting:await meeting(publication)});
        const {code}=await r.json();
        await database.prepare('UPDATE invitations SET created_at=?,expires_at=? WHERE publication=?').bind(fixed-24*3600,fixed+1,publication).run();
        assert.equal((await dispatch(host,'update',{code,duration_hours:24})).status,409);
        assert.equal((await dispatch(guest,'resolve',{code})).status,200);
        await database.prepare('UPDATE invitations SET expires_at=? WHERE publication=?').bind(fixed,publication).run();
        assert.equal((await dispatch(guest,'resolve',{code})).status,410);
    } finally {await local.dispose();}
});
test('mide sentencias D1 reales por crear, resolver y aceptar',async(t)=>{
    let index=await readFile(new URL('../src/index.js',import.meta.url),'utf8');
    index=index.replace('async fetch(request,env) {',`async fetch(request,env) {
        let statements=0;const native=env.DB;env={...env,DB:{prepare(sql){statements++;return native.prepare(sql);},batch(s){return native.batch(s);}}};`);
    index=index.replace('return json(await execute(route,env.DB,identity,data,now));',`const response=json(await execute(route,env.DB,identity,data,now));response.headers.set('X-Test-Statements',String(statements));return response;`);
    const security=await readFile(new URL('../src/security.js',import.meta.url),'utf8');
    const local=new Miniflare({modules:[{type:'ESModule',path:'index.js',contents:index},{type:'ESModule',path:'security.js',contents:security}],d1Databases:['DB'],bindings:{}});
    try {
        const database=await local.getD1Database('DB');
        await database.exec((await readFile(new URL('../migrations/0001_invitaciones.sql',import.meta.url),'utf8')).replace(/\r?\n/g,' '));
        const publication=randomBytes(16).toString('hex');
        const dispatch=async(who,route,data)=>local.dispatchFetch(`http://localhost/v1/${route}`,{method:'POST',body:JSON.stringify(await signed(who,`/v1/${route}`,data))});
        const created=await dispatch(host,'create',{publication,duration_hours:24,meeting:await meeting(publication)});
        const invite=await created.json(); assert.equal(created.status,200);
        const resolved=await dispatch(guest,'resolve',{code:invite.code});assert.equal(resolved.status,200);
        const claimed=await dispatch(guest,'claim',{code:invite.code});assert.equal(claimed.status,200);
        const retry=await dispatch(guest,'claim',{code:invite.code});assert.equal(retry.status,200);
        const counts=[created,resolved,claimed,retry].map(r=>Number(r.headers.get('X-Test-Statements')));
        assert.deepEqual(counts,[11,7,8,8]);
        t.diagnostic(`sentencias D1 crear/resolver/aceptar/reintentar: ${counts.join('/')}; trigger de cupo adicional dentro de SQLite`);
    } finally {await local.dispose();}
});
