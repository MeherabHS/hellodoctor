'use client';
import {createContext,useCallback,useContext,useEffect,useLayoutEffect,useMemo,useRef,useState,type ReactNode} from 'react';

type Blocker={dirty:boolean;pending:boolean};
type Navigation=()=>void|Promise<void>;
type Guard={update:(key:symbol,value:Blocker)=>void;remove:(key:symbol)=>void;requestNavigation:(action:Navigation)=>void};
const Context=createContext<Guard|null>(null);

export function NavigationGuard({children}:{children:ReactNode}) {
  const blockers=useRef(new Map<symbol,Blocker>()),destination=useRef<Navigation|null>(null),allowUnload=useRef(false);
  const [flags,setFlags]=useState({unsafe:false,pending:false}),[dialog,setDialog]=useState(false);
  const refresh=useCallback(()=>{
    const values=[...blockers.current.values()];const unsafe=values.some(v=>v.dirty||v.pending),pending=values.some(v=>v.pending);
    setFlags(old=>old.unsafe===unsafe&&old.pending===pending?old:{unsafe,pending});
  },[]);
  const update=useCallback((key:symbol,value:Blocker)=>{blockers.current.set(key,value);refresh();},[refresh]);
  const remove=useCallback((key:symbol)=>{blockers.current.delete(key);refresh();},[refresh]);
  const requestNavigation=useCallback((action:Navigation)=>{
    if([...blockers.current.values()].some(v=>v.dirty||v.pending)){destination.current=action;setDialog(true);}
    else void action();
  },[]);
  const guard=useMemo(()=>({update,remove,requestNavigation}),[update,remove,requestNavigation]);

  useEffect(()=>{
    if(typeof window==='undefined'||typeof document==='undefined')return;
    const warn=(event:BeforeUnloadEvent)=>{
      if(!allowUnload.current&&[...blockers.current.values()].some(v=>v.dirty||v.pending)){event.preventDefault();event.returnValue='';}
    };
    const link=(event:MouseEvent)=>{
      if(event.defaultPrevented||event.button!==0||event.metaKey||event.ctrlKey||event.shiftKey||event.altKey)return;
      const anchor=(event.target as Element)?.closest?.('a');
      if(!anchor||anchor.target==='_blank'||anchor.hasAttribute('download')||!anchor.href||anchor.href.startsWith('javascript:'))return;
      if(![...blockers.current.values()].some(v=>v.dirty||v.pending))return;
      event.preventDefault();event.stopPropagation();requestNavigation(()=>{allowUnload.current=true;window.location.assign(anchor.href);});
    };
    window.addEventListener('beforeunload',warn);document.addEventListener('click',link,true);
    return()=>{window.removeEventListener('beforeunload',warn);document.removeEventListener('click',link,true);};
  },[requestNavigation]);

  useEffect(()=>{
    if(typeof window==='undefined'||typeof document==='undefined'||!flags.unsafe||window.history.length<2)return;
    // An opaque, same-URL history checkpoint catches Next client-side Back as
    // well as browser traversal. No article, question or credential is stored.
    const key=crypto.randomUUID();let restoring=false,leaving=false;
    window.history.pushState({...window.history.state,hdEditorGuard:key},'',window.location.href);
    const pop=(event:PopStateEvent)=>{
      event.stopImmediatePropagation();
      if(event.state?.hdEditorGuard===key){
        if(restoring){restoring=false;requestNavigation(()=>{leaving=true;allowUnload.current=true;window.removeEventListener('popstate',pop,true);window.history.go(-2);});}
        return;
      }
      restoring=true;window.history.forward();
    };
    window.addEventListener('popstate',pop,true);
    return()=>{
      window.removeEventListener('popstate',pop,true);
      if(!leaving&&window.history.state?.hdEditorGuard===key){
        const suppress=(event:PopStateEvent)=>event.stopImmediatePropagation();
        window.addEventListener('popstate',suppress,{capture:true,once:true});window.history.back();
      }
    };
  },[flags.unsafe,requestNavigation]);

  function stay(){destination.current=null;setDialog(false);}
  function leave(){
    if([...blockers.current.values()].some(v=>v.pending))return;
    const action=destination.current;destination.current=null;setDialog(false);if(action)void action();
  }
  return <Context.Provider value={guard}>{children}{dialog&&<div className="admin-modal-backdrop" style={{zIndex:10000}}><section role="alertdialog" aria-modal="true" aria-labelledby="unsaved-title" aria-describedby="unsaved-description" className="admin-modal-card" style={{maxWidth:480,padding:24}}>
    <h2 id="unsaved-title">Leave this editor?</h2><p id="unsaved-description">{flags.pending?'An editor operation is still running. Stay until it finishes before leaving.':'You have unsaved content or an unconfirmed import. Discard it only if you intend to leave.'}</p>
    <div className="cms-actions"><button type="button" autoFocus onClick={stay}>Stay</button><button type="button" disabled={flags.pending} onClick={leave}>Discard and Leave</button></div>
  </section></div>}</Context.Provider>;
}

export function useAdminNavigation(){
  const guard=useContext(Context);
  if(!guard)throw new Error('Admin navigation requires NavigationGuard.');
  return guard;
}
export function useUnsavedChanges(dirty:boolean,pending=false){
  const guard=useContext(Context),key=useRef(Symbol('editor'));
  useLayoutEffect(()=>{guard?.update(key.current,{dirty,pending});},[guard,dirty,pending]);
  useLayoutEffect(()=>{const id=key.current;return()=>guard?.remove(id);},[guard]);
}
