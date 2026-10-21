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
