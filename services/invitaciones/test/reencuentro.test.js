import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { Miniflare } from 'miniflare';
import { randomBytes } from 'node:crypto';

// Mismo fixture que invitaciones.test.js; se duplica para no tocar ese archivo.
let mf, db, host;
const hex = b => Buffer.from(b).toString('hex');
async function identity() {
    const pair = await crypto.subtle.generateKey('Ed25519', true, ['sign', 'verify']);
    return { ...pair, public_key: hex(await crypto.subtle.exportKey('raw', pair.publicKey)) };
}
async function signed(who, path, data) {
    const e = { public_key: who.public_key, nonce: randomBytes(16).toString('hex'), issued_at: Math.floor(Date.now()/1000), payload: JSON.stringify(data) };
    e.signature = hex(await crypto.subtle.sign('Ed25519', who.privateKey, new TextEncoder().encode(`icaro-invitaciones/1\nPOST\n${path}\n${e.public_key}\n${e.nonce}\n${e.issued_at}\n${e.payload}`)));
    return e;
}
async function send(who, route, data) {
    const envelope = await signed(who, `/v1/${route}`, data);
    return mf.dispatchFetch(`http://localhost/v1/${route}`, {method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(envelope)});
}
async function meeting(publication, hint = 'iroh:test-local', signer = host) {
    const payload = JSON.stringify({host: signer.public_key, publication, endpoint_hints:[hint], issued_at:Math.floor(Date.now()/1000)});
    const signature = hex(await crypto.subtle.sign('Ed25519', signer.privateKey, new TextEncoder().encode(`icaro-encuentro/1\n${payload}`)));
    return {payload,signature};
}
async function create(capacity=10, publication=randomBytes(16).toString('hex'), m=undefined) {
    const response = await send(host,'create',{publication,capacity,duration_hours:24,meeting:m??await meeting(publication)});
    assert.equal(response.status,200,await response.clone().text());
    return {...await response.json(),publication};
}
const reconnect = (who, publication, h = host.public_key) => send(who,'reconnect',{host:h,publication});
async function expectError(response, status, code) {
    assert.equal(response.status,status);
    assert.equal((await response.json()).error,code);
}
const used = async publication => (await db.prepare('SELECT COALESCE(SUM(used),0) AS n FROM invitations WHERE publication=?').bind(publication).first()).n;
const authorizations = async publication => (await db.prepare('SELECT COUNT(*) AS n FROM authorizations WHERE publication=?').bind(publication).first()).n;

before(async()=> {
    mf = new Miniflare({modules:true,modulesRules:[{type:'ESModule',include:['**/*.js']}],scriptPath:new URL('../src/index.js',import.meta.url).pathname.replace(/^\/([A-Z]:)/,'$1'),d1Databases:['DB'],bindings:{RATE_PER_MINUTE:'10000',DAILY_REQUEST_BUDGET:'100000'}});
    db = await mf.getD1Database('DB');
    await db.exec((await readFile(new URL('../migrations/0001_invitaciones.sql',import.meta.url),'utf8')).replace(/\r?\n/g,' '));
    host=await identity();
});
after(async()=>{await mf?.dispose();});

test('aceptado se reencuentra tras caducar, revocar y renovar el código',async()=>{
    const invite=await create(1),accepted=await identity();
    assert.equal((await send(accepted,'claim',{code:invite.code})).status,200);
    const first=await reconnect(accepted,invite.publication);
    assert.equal(first.status,200);
    const body=await first.json();
    assert.deepEqual(Object.keys(body).sort(),['host','meeting','publication']);
    assert.equal(body.host,host.public_key); assert.equal(body.publication,invite.publication);
    assert.equal(JSON.parse(body.meeting.payload).publication,invite.publication);
    await db.prepare('UPDATE invitations SET expires_at=1 WHERE publication=?').bind(invite.publication).run();
    assert.equal((await reconnect(accepted,invite.publication)).status,200);
    assert.equal((await send(host,'revoke-code',{code:invite.code})).status,200);
    assert.equal((await reconnect(accepted,invite.publication)).status,200);
    const renewedMeeting=await meeting(invite.publication,'iroh:renovado');
    await create(1,invite.publication,renewedMeeting);
    const renewed=await reconnect(accepted,invite.publication);
    assert.equal(renewed.status,200);
    assert.deepEqual((await renewed.json()).meeting,renewedMeeting);
    assert.equal(await used(invite.publication),1);
    assert.equal(await authorizations(invite.publication),1);
});
test('devuelve el encuentro vigente tras un ajuste del anfitrión',async()=>{
    const invite=await create(2),accepted=await identity();
    await send(accepted,'claim',{code:invite.code});
    const updated=await meeting(invite.publication,'iroh:actualizado');
    assert.equal((await send(host,'update',{code:invite.code,capacity:2,duration_hours:48,meeting:updated})).status,200);
    const r=await reconnect(accepted,invite.publication);
    assert.equal(r.status,200); assert.deepEqual((await r.json()).meeting,updated);
});
test('desconocidos, otro anfitrión y el propio anfitrión no heredan acceso ni consumen cupo',async()=>{
    const invite=await create(2),accepted=await identity(),stranger=await identity(),other=await identity();
    await send(accepted,'claim',{code:invite.code});
    const previous=await used(invite.publication);
    await expectError(await reconnect(stranger,invite.publication),403,'access_revoked');
    await expectError(await reconnect(host,invite.publication),403,'access_revoked');
    await expectError(await reconnect(accepted,invite.publication,other.public_key),403,'wrong_host');
    await expectError(await reconnect(accepted,randomBytes(16).toString('hex')),404,'not_found');
    // Otro anfitrión con su propia publicación no abre la ajena.
    const foreign=randomBytes(16).toString('hex');
    await send(other,'create',{publication:foreign,capacity:5,duration_hours:24,meeting:await meeting(foreign,'iroh:otro',other)});
    await expectError(await reconnect(accepted,foreign,other.public_key),403,'access_revoked');
    await expectError(await reconnect(accepted,foreign,host.public_key),403,'wrong_host');
    assert.equal(await used(invite.publication),previous);
    assert.equal(await authorizations(invite.publication),1);
    assert.equal(await used(foreign),0);
    assert.equal(await authorizations(foreign),0);
});
test('rechaza identificadores no hexadecimales',async()=>{
    const guest=await identity(),publication=randomBytes(16).toString('hex');
    for(const data of [{host:host.public_key.toUpperCase(),publication},{host:host.public_key,publication:publication.slice(1)},{publication},{host:host.public_key}])
        assert.equal((await send(guest,'reconnect',data)).status,400);
});
test('revocar la identidad corta el reencuentro aunque el código siga vigente',async()=>{
    const invite=await create(2),accepted=await identity();
    await send(accepted,'claim',{code:invite.code});
    assert.equal((await send(host,'revoke-access',{publication:invite.publication,guest:accepted.public_key})).status,200);
    await expectError(await reconnect(accepted,invite.publication),403,'access_revoked');
    assert.equal(await used(invite.publication),1);
});
test('detener devuelve sharing_stopped y reabrir exige nueva aceptación',async()=>{
    const invite=await create(2),accepted=await identity();
    await send(accepted,'claim',{code:invite.code});
    assert.equal((await send(host,'stop',{publication:invite.publication})).status,200);
    await expectError(await reconnect(accepted,invite.publication),410,'sharing_stopped');
    const reopened=await create(2,invite.publication);
    await expectError(await reconnect(accepted,invite.publication),403,'access_revoked');
    assert.equal(await authorizations(invite.publication),0);
    assert.equal((await send(accepted,'claim',{code:reopened.code})).status,200);
    assert.equal((await reconnect(accepted,invite.publication)).status,200);
});
test('reencuentro respeta nonces y no sustituye el presupuesto por identidad',async()=>{
    const invite=await create(1),accepted=await identity();
    await send(accepted,'claim',{code:invite.code});
    const envelope=await signed(accepted,'/v1/reconnect',{host:host.public_key,publication:invite.publication});
    const dispatch=()=>mf.dispatchFetch('http://localhost/v1/reconnect',{method:'POST',body:JSON.stringify(envelope)});
    assert.equal((await dispatch()).status,200);
    await expectError(await dispatch(),409,'replayed_nonce');
    // La firma cubre la ruta: un sobre de resolve no sirve para reconnect.
    const other=await signed(accepted,'/v1/resolve',{host:host.public_key,publication:invite.publication});
    assert.equal((await mf.dispatchFetch('http://localhost/v1/reconnect',{method:'POST',body:JSON.stringify(other)})).status,401);
    const minute=Math.floor(Date.now()/60000);
    for(const bucket of [minute,minute+1]) await db.prepare('INSERT INTO rate_limits(bucket,n,expires_at) VALUES(?,60,?) ON CONFLICT(bucket) DO UPDATE SET n=60').bind(`identity:${bucket}:${accepted.public_key}`,Math.floor(Date.now()/1000)+120).run();
    await expectError(await reconnect(accepted,invite.publication),429,'identity_rate_limited');
});
