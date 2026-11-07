import {HttpError,requireThat,isHex,digest,normalizeCode,newCode,authenticate,validateMeeting,rateLimit} from './security.js';

const routes=new Set(['create','resolve','claim','update','revoke-code','revoke-access','stop','authorizations']);
function settings(data) {
    const capacity=data.capacity??10;
    requireThat(Number.isSafeInteger(capacity) && capacity>=1 && capacity<=4294967295);
    requireThat([24,48,168].includes(data.duration_hours));
    return capacity;
}
async function publication(db,id,identity) {
    requireThat(isHex(id,32));
    const p=await db.prepare('SELECT * FROM publications WHERE publication=?').bind(id).first();
    requireThat(p,404,'not_found'); requireThat(p.host===identity,403,'wrong_host');
    return p;
}
async function invitation(db,code) {
    const hash=await digest(normalizeCode(code));
    const row=await db.prepare('SELECT i.*,p.host,p.meeting,p.active FROM invitations i JOIN publications p USING(publication) WHERE code_hash=?').bind(hash).first();
    requireThat(row,404,'not_found'); return row;
}
async function describe(db,row,identity) {
    const auth=await db.prepare('SELECT revoked FROM authorizations WHERE publication=? AND guest=?').bind(row.publication,identity).first();
    return {host:row.host,publication:row.publication,meeting:JSON.parse(row.meeting),expires_at:row.expires_at,capacity:row.capacity,accepted:row.used,authorized:!!auth&&!auth.revoked&&!!row.active};
}
async function execute(route,db,identity,data,now) {
    if(route==='create') {
        requireThat(isHex(data.publication,32)); const capacity=settings(data);
        await validateMeeting(data.meeting,identity,data.publication,now);
        const existing=await db.prepare('SELECT * FROM publications WHERE publication=?').bind(data.publication).first();
        if(existing) requireThat(existing.host===identity,403,'wrong_host');
        const count=await db.prepare('SELECT COUNT(*) AS n FROM invitations i JOIN publications p USING(publication) WHERE p.host=? AND i.publication<>? AND i.revoked=0 AND i.expires_at>?').bind(identity,data.publication,now).first();
        requireThat(count.n<100,429,'host_invitation_limit');
        for(let attempt=0;attempt<4;attempt++) {
            const code=newCode(),hash=await digest(code),expiry=now+data.duration_hours*3600;
            let results;
            try {
                // Plain INSERT deliberately rolls the entire batch back on collision:
                // previous code, meeting and generation remain untouched on failure.
                results=await db.batch([
                    db.prepare('INSERT INTO publications(publication,host,meeting) VALUES(?,?,?) ON CONFLICT DO NOTHING').bind(data.publication,identity,JSON.stringify(data.meeting)),
                    db.prepare('INSERT INTO invitations(code_hash,publication,created_at,expires_at,capacity) SELECT ?,publication,?,?,? FROM publications WHERE publication=? AND host=?').bind(hash,now,expiry,capacity,data.publication,identity),
                    // Only explicit restart after stop clears the previous generation.
                    // Active renewal preserves accepted guests and revocation tombstones.
                    db.prepare('DELETE FROM authorizations WHERE publication=? AND EXISTS(SELECT 1 FROM publications WHERE publication=? AND host=? AND active=0)').bind(data.publication,data.publication,identity),
                    db.prepare('UPDATE invitations SET revoked=1 WHERE publication=? AND code_hash<>? AND EXISTS(SELECT 1 FROM publications WHERE publication=? AND host=?)').bind(data.publication,hash,data.publication,identity),
                    db.prepare('UPDATE publications SET meeting=?,active=1 WHERE publication=? AND host=?').bind(JSON.stringify(data.meeting),data.publication,identity)
                ]);
            } catch(error) {
                if(/UNIQUE constraint failed: invitations.code_hash/.test(String(error))) continue;
                throw error;
            }
            if(results[1].meta.changes===1) return {code,expires_at:expiry,capacity,accepted:0};
            const current=await db.prepare('SELECT host,active FROM publications WHERE publication=?').bind(data.publication).first();
            requireThat(current?.host===identity,403,'wrong_host');
        }
        throw new HttpError(503,'code_collision');
    }
    if(['authorizations','revoke-access','stop'].includes(route)) {
        await publication(db,data.publication,identity);
        if(route==='authorizations') {
            // Paginate to keep control messages bounded, regardless of product capacity.
            const after=data.after??''; requireThat(after===''||isHex(after,64));
            const p=await publication(db,data.publication,identity);
            const rows=await db.prepare('SELECT guest FROM authorizations WHERE publication=? AND revoked=0 AND guest>? ORDER BY guest LIMIT 101').bind(data.publication,after).all();
            const guests=rows.results.slice(0,100).map(r=>r.guest);
            return {publication:data.publication,active:!!p.active,guests,next:rows.results.length>100?guests.at(-1):null};
        }
        if(route==='stop') {
            await db.batch([
                db.prepare('UPDATE publications SET active=0 WHERE publication=? AND host=?').bind(data.publication,identity),
                db.prepare('UPDATE authorizations SET revoked=1 WHERE publication=?').bind(data.publication),
                db.prepare('UPDATE invitations SET revoked=1 WHERE publication=?').bind(data.publication)
            ]);
        } else {
            requireThat(isHex(data.guest,64));
            // Tombstone even a guest whose acceptance races this revocation.
            await db.prepare('INSERT INTO authorizations(publication,guest,code_hash,accepted_at,revoked) SELECT ?,?,code_hash,?,1 FROM invitations WHERE publication=? LIMIT 1 ON CONFLICT(publication,guest) DO UPDATE SET revoked=1').bind(data.publication,data.guest,now,data.publication).run();
        }
        return {ok:true};
    }
    let row=await invitation(db,data.code);
    if(route==='update'||route==='revoke-code') {
        requireThat(row.host===identity,403,'wrong_host');
        if(route==='revoke-code') {
            await db.prepare('UPDATE invitations SET revoked=1 WHERE code_hash=?').bind(row.code_hash).run();
            return {ok:true};
        }
        const capacity=settings(data),expiry=row.created_at+data.duration_hours*3600;
        requireThat(!row.revoked&&row.expires_at>now&&row.active,410,'invitation_expired');
        requireThat(expiry>now,409,'expiry_in_past');
        if(data.meeting) await validateMeeting(data.meeting,identity,row.publication,now);
        const result=await db.prepare('UPDATE invitations SET capacity=?,expires_at=? WHERE code_hash=? AND used<=? AND revoked=0 AND expires_at>? AND EXISTS(SELECT 1 FROM publications WHERE publication=? AND active=1)').bind(capacity,expiry,row.code_hash,capacity,now,row.publication).run();
        requireThat(result.meta.changes===1,409,'capacity_below_accepted');
        if(data.meeting) await db.prepare('UPDATE publications SET meeting=? WHERE publication=? AND host=? AND active=1').bind(JSON.stringify(data.meeting),row.publication,identity).run();
        return describe(db,await invitation(db,data.code),identity);
    }
    if(route==='claim') {
        // A single SQLite statement tests availability and inserts. Trigger increments
        // used in the same transaction; uniqueness makes retries capacity-neutral.
        await db.prepare(`INSERT OR IGNORE INTO authorizations(publication,guest,code_hash,accepted_at)
            SELECT i.publication,?,i.code_hash,? FROM invitations i JOIN publications p USING(publication)
            WHERE i.code_hash=? AND i.revoked=0 AND i.expires_at>? AND i.used<i.capacity AND p.active=1`)
            .bind(identity,now,row.code_hash,now).run();
        row=await invitation(db,data.code);
    }
    requireThat(row.active,410,'sharing_stopped');
    const answer=await describe(db,row,identity);
    if(answer.authorized) return answer;
    const denied=await db.prepare('SELECT revoked FROM authorizations WHERE publication=? AND guest=?').bind(row.publication,identity).first();
    requireThat(!denied?.revoked,403,'access_revoked');
    requireThat(!row.revoked&&row.expires_at>now,410,'invitation_expired');
    requireThat(row.used<row.capacity,409,'capacity_full');
    requireThat(route!=='claim',409,'claim_conflict');
    return answer;
}
const json=(value,status=200)=>Response.json(value,{status,headers:{'Cache-Control':'no-store','X-Content-Type-Options':'nosniff',...(status===429?{'Retry-After':'60'}:{})}});
export default {
    async fetch(request,env) {
        try {
            const url=new URL(request.url),route=url.pathname.slice(4);
            requireThat(request.method==='POST'&&url.pathname.startsWith('/v1/')&&routes.has(route)&&!url.search,404,'not_found');
            const now=Math.floor(Date.now()/1000);
            await rateLimit(env.DB,request,env,now);
            const {identity,data}=await authenticate(request,env.DB,now);
            return json(await execute(route,env.DB,identity,data,now));
        } catch(error) {
            // Do not log requests, codes, identity payloads or database details.
            return json({error:error instanceof HttpError?error.message:'temporarily_unavailable'},error instanceof HttpError?error.status:503);
        }
    },
    async scheduled(_event,env) {
        const now=Math.floor(Date.now()/1000);
        await env.DB.batch([
            env.DB.prepare('DELETE FROM nonces WHERE expires_at<?').bind(now),
            env.DB.prepare('DELETE FROM rate_limits WHERE expires_at<?').bind(now)
        ]);
    }
};
