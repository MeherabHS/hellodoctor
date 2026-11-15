export function createInflightReads() {
 /** @type {Map<string,Promise<unknown>>} */
 const jobs=new Map();
 return {
  /** @template T @param {string} key @param {()=>Promise<T>} load @returns {Promise<T>} */
  run(key,load) {
   const found=jobs.get(key);if(found)return /** @type {Promise<T>} */ (found);
   const work=Promise.resolve().then(load).finally(()=>{if(jobs.get(key)===work)jobs.delete(key);});
   jobs.set(key,work);return work;
  },
  clear(){jobs.clear();}
 };
}

/**
 * Soft read cache (2026-11-14): a successful GET is reused for `ttlMs` so
 * the same read fired by several components, tab switches or quick
 * navigation does not reach the server again. Failures are never cached.
 * Callers clear it on every write, sign-out and account change. Values live
 * only in this tab's memory and are deep-copied on every hit, so no caller
 * can change another caller's data.
 * @param {number} ttlMs @param {() => number} [now]
 */
export function createSoftReads(ttlMs, now = () => Date.now()) {
 /** @type {Map<string,{at:number,value:unknown}>} */
 const done=new Map();
 const inflight=createInflightReads();
 let generation=0;
 const copy=(/** @type {unknown} */ v)=>typeof structuredClone==='function'?structuredClone(v):JSON.parse(JSON.stringify(v));
 return {
  /** @template T @param {string} key @param {()=>Promise<T>} load @returns {Promise<T>} */
  run(key,load) {
   const hit=done.get(key);
   if(hit&&now()-hit.at<ttlMs)return Promise.resolve(/** @type {T} */ (copy(hit.value)));
   if(hit)done.delete(key);
   const started=generation;
   return inflight.run(key,async()=>{
    const value=await load();
    if(started===generation){
     if(done.size>=200)done.delete(done.keys().next().value);
     done.set(key,{at:now(),value:copy(value)});
    }
    return value;
   });
  },
  clear(){generation++;done.clear();inflight.clear();}
 };
}
