import React from 'react';
import test from 'node:test';
import assert from 'node:assert/strict';
import {act,create,type ReactTestRenderer} from 'react-test-renderer';
import {PeerNavigators,PEER_ACTIONS,peerDecisionNeedsNote} from '../src/components/people/PeerNavigators';
import {clearSoftReads} from '../src/lib/api';

const peer=(id:string,status:string,name:string)=>({id,full_name:name,email:`${id}@example.test`,phone_number:'+254700000000',county:'Nairobi',organization:'Youth Centre',approval_status:status,approval_note:null,reviewed_at:null,created_at:'2026-11-15T08:00:00Z'});
const ok=(data:unknown)=>new Response(JSON.stringify({success:true,data}));
const text=(v:ReactTestRenderer)=>JSON.stringify(v.toJSON());
type Node=string|{children:Node[]};
const words=(n:Node):string=>typeof n==='string'?n:n.children.map(words).join('');
const button=(v:ReactTestRenderer,label:string)=>v.root.findAllByType('button').find(b=>words(b as unknown as Node).includes(label))!;

test('allowed decisions mirror the backend and reject/suspend need a note',()=>{
  assert.deepEqual(PEER_ACTIONS.PENDING,['reject','approve']);
  assert.deepEqual(PEER_ACTIONS.APPROVED,['suspend']);
  assert.deepEqual(PEER_ACTIONS.SUSPENDED,['reinstate']);
  assert.deepEqual(PEER_ACTIONS.REJECTED,['approve']);
  assert.equal(peerDecisionNeedsNote('reject'),true);assert.equal(peerDecisionNeedsNote('suspend'),true);
  assert.equal(peerDecisionNeedsNote('approve'),false);assert.equal(peerDecisionNeedsNote('reinstate'),false);
});

test('pending peers are listed; reject needs a note; approve posts the decision',async()=>{
  clearSoftReads();
  const previous=globalThis.fetch;const calls:{url:string;method:string;body:unknown}[]=[];
  let list=[peer('p1','PENDING','Grace Wanjiku'),peer('p2','APPROVED','Brian Otieno')];
  globalThis.fetch=async(url,init)=>{const method=init?.method||'GET';calls.push({url:String(url),method,body:init?.body?JSON.parse(String(init.body)):null});
    if(method==='POST'){list=list.map(p=>p.id==='p1'?{...p,approval_status:'APPROVED'}:p);return ok(list[0]);}
    return ok(list);};
  let view:ReactTestRenderer|undefined;
  try{
    await act(async()=>{view=create(<PeerNavigators/>);});
    assert.ok(text(view!).includes('Grace Wanjiku'));
    assert.ok(!text(view!).includes('Brian Otieno'),'the Pending tab hides approved peers');
    await act(async()=>{view!.root.findAllByProps({className:'admin-clickable-row'})[0].props.onClick();});
    assert.ok(text(view!).includes('Back to Peer Navigators'));
    await act(async()=>{button(view!,'Reject').props.onClick();});
    assert.ok(text(view!).includes('Add a short note'));
    assert.equal(calls.filter(c=>c.method==='POST').length,0);
    await act(async()=>{button(view!,'Approve').props.onClick();});
    const post=calls.find(c=>c.method==='POST')!;
    assert.ok(post.url.endsWith('/admin/peers/p1/approve'),post.url);
    assert.deepEqual(post.body,{});
    assert.ok(text(view!).includes('Pending review'));
  }finally{if(view)act(()=>view!.unmount());globalThis.fetch=previous;}
});
