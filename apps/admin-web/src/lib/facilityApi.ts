import {request} from './api';
import type {AdminFacility,FacilityContent,ServiceCategory} from './facilityContent';

export const listFacilities=(query:URLSearchParams)=>request<{items:AdminFacility[];total:number}>(`/admin/facilities?${query}`);
export const getFacility=(id:string)=>request<AdminFacility>(`/admin/facilities/${encodeURIComponent(id)}`);
export const facilityOptions=()=>request<{services:ServiceCategory[];counties:string[]}>('/admin/facility-services');
export const createFacility=(content:FacilityContent)=>request<AdminFacility>('/admin/facilities',{method:'POST',body:JSON.stringify({content})});
export const updateFacility=(id:string,version:number,content:FacilityContent)=>request<AdminFacility>(`/admin/facilities/${encodeURIComponent(id)}`,{method:'PATCH',body:JSON.stringify({version,content})});
export const transitionFacility=(id:string,version:number,action:'publish'|'unpublish'|'archive')=>request<AdminFacility>(`/admin/facilities/${encodeURIComponent(id)}/${action}`,{method:'POST',body:JSON.stringify({version})});

/** Photo preview through the authenticated binary route (never a storage URL). */
export const facilityImageUrl=(id:string,imageVersion:string|null)=>`/api/facility-media/${encodeURIComponent(id)}${imageVersion?`?v=${encodeURIComponent(imageVersion)}`:''}`;

async function binary(id:string,method:'POST'|'DELETE',version:number,file?:File):Promise<AdminFacility>{
  const res=await fetch(`/api/facility-media/${encodeURIComponent(id)}?version=${version}`,{method,credentials:'include',headers:file?{'Content-Type':file.type}:{},body:file});
  const data=await res.json().catch(()=>({}));
  if(!res.ok||!data.success)throw new Error(data.error?.message||data.error||'Photo update failed.');
  return data.data;
}
export async function uploadFacilityImage(id:string,version:number,file:File):Promise<AdminFacility>{
  if(!['image/jpeg','image/png'].includes(file.type)||file.size>5*1024*1024)throw new Error('Choose a JPEG or PNG up to 5 MB.');
  return binary(id,'POST',version,file);
}
export const removeFacilityImage=(id:string,version:number)=>binary(id,'DELETE',version);
