import {useEffect, useRef, useState} from 'react';
import {Map as GlMap, Marker, NavigationControl, setWorkerUrl, type GeoJSONSource} from 'maplibre-gl';
import workerUrl from 'maplibre-gl/dist/maplibre-gl-worker.mjs?worker&url';
import 'maplibre-gl/dist/maplibre-gl.css';
import type {Point} from '@meridian/shared-types';
import {formatPoint} from '@meridian/maps';

type Props = {point:Point;held:Point|null;route:Point[];online:boolean;onSelect:(point:Point)=>void};
type LoadState = {kind:'loading'|'ready'|'error';message?:string};
const MAX_MAP_LATITUDE = 85.05112878;
const ROUTE_SOURCE = 'meridian-route';
const LOAD_DEADLINE_MS = 30_000;
function valid(point:Point):boolean {
  return Number.isFinite(point.latitude) && Number.isFinite(point.longitude)
    && Math.abs(point.latitude)<=90 && Math.abs(point.longitude)<=180;
}
function displayed(point:Point):[number,number] {
  return [point.longitude,Math.max(-MAX_MAP_LATITUDE,Math.min(MAX_MAP_LATITUDE,point.latitude))];
}
function routeData(route:Point[]) {
  // Empty collections clear drafts with fewer than two points without sending
  // malformed LineStrings to the map worker.
  const features=route.length>=2&&route.every(valid)?[{
    type:'Feature' as const,properties:{},geometry:{type:'LineString' as const,coordinates:route.map(displayed)},
  }]:[];
  return {type:'FeatureCollection' as const,features};
}
function updateRoute(map:GlMap,route:Point[]) {
  if(!map.isStyleLoaded())return;
  const data=routeData(route),source=map.getSource(ROUTE_SOURCE) as GeoJSONSource|undefined;
  if(source)source.setData(data);
  else {
    map.addSource(ROUTE_SOURCE,{type:'geojson',data});
    map.addLayer({id:'meridian-route-line',type:'line',source:ROUTE_SOURCE,paint:{'line-color':'#bdf774','line-width':4}});
  }
}
export default function MapView(props:Props){
  const {point,held,route,online}=props;
  const container=useRef<HTMLDivElement>(null),map=useRef<GlMap|null>(null);
  const selected=useRef<Marker|null>(null),active=useRef<Marker|null>(null),latest=useRef(props);
  const [load,setLoad]=useState<LoadState>({kind:'loading'}),[attempt,setAttempt]=useState(0);
  latest.current=props;
  useEffect(()=>{
    if(!online||!container.current)return;
    let current:GlMap|undefined,disposed=false,failed=false;
    let deadline:ReturnType<typeof setTimeout>|undefined;
    const stopDeadline=()=>{if(deadline!==undefined)clearTimeout(deadline);deadline=undefined;};
    const fail=(message='Street maps could not load. Retry or switch to the offline coordinate grid.')=>{
      if(disposed)return;failed=true;stopDeadline();setLoad({kind:'error',message});
    };
    setLoad({kind:'loading'});
    try {
      // Vite must explicitly emit MapLibre 6's worker and bundle its shared imports.
      setWorkerUrl(workerUrl);
      const initial=valid(latest.current.point)?latest.current.point:{latitude:0,longitude:0};
      current=new GlMap({container:container.current,style:'https://tiles.openfreemap.org/styles/dark',center:displayed(initial),zoom:12,attributionControl:{compact:true}});
      map.current=current;
      current.addControl(new NavigationControl(),'bottom-right');
      selected.current=new Marker({color:'#bdf774'}).setLngLat(displayed(initial)).addTo(current);
      active.current=new Marker({color:'#55ccff'});
      if(latest.current.held&&valid(latest.current.held))active.current.setLngLat(displayed(latest.current.held)).addTo(current);
      current.on('click',event=>{
        const chosen={latitude:event.lngLat.lat,longitude:((event.lngLat.lng+180)%360+360)%360-180};
        if(valid(chosen))latest.current.onSelect(chosen);
      });
      current.on('error',()=>fail());
      current.on('webglcontextlost',()=>fail('Street map rendering was interrupted. Retry the map or use the offline coordinate grid.'));
      const syncRoute=()=>{if(!disposed&&current)updateRoute(current,latest.current.route);};
      current.on('style.load',syncRoute);
      current.on('load',()=>{
        if(disposed||!current)return;
        stopDeadline();current.resize();syncRoute();
        if(!failed)setLoad({kind:'ready'});
      });
      deadline=setTimeout(()=>fail('Street maps are taking too long to load. Retry or use the offline coordinate grid.'),LOAD_DEADLINE_MS);
    }catch {fail('Street maps could not start. Retry or use the offline coordinate grid.');}
    return()=>{
      disposed=true;stopDeadline();
      selected.current?.remove();active.current?.remove();current?.remove();
      if(map.current===current)map.current=null;
      selected.current=null;active.current=null;
    };
  },[online,attempt]);
  useEffect(()=>{
    if(!valid(point))return;
    selected.current?.setLngLat(displayed(point));
    const reduced=window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
    map.current?.easeTo({center:displayed(point),duration:reduced?0:350});
  },[point,online,attempt]);
  useEffect(()=>{
    if(held&&valid(held)&&map.current)active.current?.setLngLat(displayed(held)).addTo(map.current);
    else active.current?.remove();
  },[held,online,attempt]);
  useEffect(()=>{if(map.current)updateRoute(map.current,route);},[route,online,attempt]);
  if(online)return <div className="map">
    <div className="map" ref={container} aria-label="Interactive street map. Click to select a destination."/>
    {load.kind!=='ready'&&<div role={load.kind==='error'?'alert':'status'} aria-live="polite" style={{position:'absolute',zIndex:2,top:102,left:24,right:24,padding:14,border:'1px solid #526248',borderRadius:8,background:'#151e18f5',fontSize:12,lineHeight:1.5}}>
      {load.kind==='loading'?'Loading street maps…':load.message}
      {load.kind==='error'&&<button className="button secondary" style={{marginLeft:12}} onClick={()=>setAttempt(old=>old+1)}>Retry street maps</button>}
    </div>}
    {valid(point)&&Math.abs(point.latitude)>MAX_MAP_LATITUDE&&<div role="status" style={{position:'absolute',left:24,right:24,bottom:92,padding:10,background:'#151e18ef',fontSize:11}}>Street maps stop near 85° latitude. Your selected coordinates are unchanged.</div>}
  </div>;
  return <div className="offline-map" onClick={event=>{
    const bounds=event.currentTarget.getBoundingClientRect();if(!bounds.width||!bounds.height)return;
    const chosen={latitude:Math.max(-90,Math.min(90,90-(event.clientY-bounds.top)/bounds.height*180)),longitude:Math.max(-180,Math.min(180,(event.clientX-bounds.left)/bounds.width*360-180))};
    latest.current.onSelect(chosen);
  }} aria-label="Offline coordinate selection grid">
    <div className="grid-label top">90° N</div><div className="grid-label bottom">90° S</div><div className="grid-label left">180° W</div><div className="grid-label right">180° E</div>
    <div className="atlas-caption"><span>MERIDIAN / COORDINATE VIEW</span><h2>A place to test<br/>every possibility.</h2><p>Choose coordinates or enable street maps.<br/>Device simulation stays local.</p></div>
    {valid(point)&&<div className="offline-pin" style={{left:`${(point.longitude+180)/360*100}%`,top:`${(90-point.latitude)/180*100}%`}}><span>+</span><small>{formatPoint(point)}</small></div>}
  </div>;
}
