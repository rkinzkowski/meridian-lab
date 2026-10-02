import {afterEach,beforeEach,describe,expect,it,vi} from 'vitest';
import {act,cleanup,fireEvent,render,screen} from '@testing-library/react';
import MapView from './MapView';

const fake=vi.hoisted(()=>({setWorkerUrl:vi.fn(),sequence:[] as string[],maps:[] as any[],markers:[] as any[],failConstruction:false}));
vi.mock('maplibre-gl/dist/maplibre-gl-worker.mjs?worker&url',()=>({default:'/bundled-map-worker.js'}));
vi.mock('maplibre-gl',()=>{
  class Map {
    handlers:Record<string,((event?:unknown)=>void)[]>={};
    sources=new globalThis.Map<string,any>();
    loaded=false;
    constructor(public options:unknown){fake.sequence.push('map');if(fake.failConstruction)throw Error('GPU unavailable');fake.maps.push(this);}
    on=vi.fn((type:string,callback:(event?:unknown)=>void)=>{(this.handlers[type]??=[]).push(callback);return this;});
    emit(type:string,event?:unknown){this.handlers[type]?.forEach(callback=>callback(event));}
    isStyleLoaded=()=>this.loaded;
    getSource=(id:string)=>this.sources.get(id);
    addSource=vi.fn((id:string,source:unknown)=>{this.sources.set(id,{initial:source,setData:vi.fn()});});
    addLayer=vi.fn();addControl=vi.fn();remove=vi.fn();resize=vi.fn();easeTo=vi.fn();
  }
  class Marker {
    constructor(public options:{color:string}){fake.markers.push(this);}
    setLngLat=vi.fn(()=>this);addTo=vi.fn(()=>this);remove=vi.fn(()=>this);
  }
  return {Map,Marker,NavigationControl:class {},setWorkerUrl:(url:string)=>{fake.sequence.push('worker');fake.setWorkerUrl(url);}};
});
const point={latitude:40.7128,longitude:-74.006},held={latitude:40.758,longitude:-73.9855};
const props={point,held:null,route:[],online:true,onSelect:vi.fn()};
beforeEach(()=>{fake.maps.length=0;fake.markers.length=0;fake.sequence.length=0;fake.failConstruction=false;vi.clearAllMocks();});
afterEach(()=>{cleanup();vi.useRealTimers();});

describe('street-map worker and lifecycle',()=>{
  it('configures the bundled worker before map construction and restores held marker on mode change',()=>{
    const view=render(<MapView {...props} held={held} online={false}/>);
    expect(fake.maps).toHaveLength(0);
    view.rerender(<MapView {...props} held={held}/>);
    expect(fake.sequence.slice(0,2)).toEqual(['worker','map']);
    expect(fake.setWorkerUrl).toHaveBeenCalledWith('/bundled-map-worker.js');
    const marker=fake.markers.find(m=>m.options.color==='#55ccff');
    expect(marker.setLngLat).toHaveBeenCalledWith([held.longitude,held.latitude]);
    expect(marker.addTo).toHaveBeenCalledWith(fake.maps[0]);
    view.rerender(<MapView {...props} held={held} online={false}/>);
    expect(fake.maps[0].remove).toHaveBeenCalledOnce();
    view.rerender(<MapView {...props} held={held}/>);
    expect(fake.markers.at(-1).addTo).toHaveBeenCalledWith(fake.maps[1]);
  });
  it('emits empty collections for zero or one waypoint and clears a previously valid route',()=>{
    const view=render(<MapView {...props} route={[point]}/>);
    const map=fake.maps[0];map.loaded=true;act(()=>map.emit('style.load'));
    const source=map.getSource('meridian-route');
    expect(source.initial.data).toEqual({type:'FeatureCollection',features:[]});
    view.rerender(<MapView {...props} route={[point,held]}/>);
    expect(source.setData.mock.lastCall[0].features[0].geometry.coordinates).toEqual([[point.longitude,point.latitude],[held.longitude,held.latitude]]);
    view.rerender(<MapView {...props} route={[]}/>);
    expect(source.setData).toHaveBeenLastCalledWith({type:'FeatureCollection',features:[]});
  });
  it('uses the latest route after asynchronous style loading',()=>{
    const view=render(<MapView {...props}/>);
    view.rerender(<MapView {...props} route={[point,held]}/>);
    const map=fake.maps[0];map.loaded=true;act(()=>map.emit('load'));
    expect(map.getSource('meridian-route').initial.data.features[0].geometry.coordinates).toHaveLength(2);
    expect(screen.queryByText('Loading street maps…')).toBeNull();
  });
  it('turns a silent worker/load hang into a bounded, retryable message',()=>{
    vi.useFakeTimers();render(<MapView {...props}/>);
    expect(screen.getByRole('status').textContent).toContain('Loading street maps');
    act(()=>vi.advanceTimersByTime(30_000));
    expect(screen.getByRole('alert').textContent).toContain('taking too long');
    fireEvent.click(screen.getByRole('button',{name:'Retry street maps'}));
    expect(fake.maps[0].remove).toHaveBeenCalledOnce();
    expect(fake.maps).toHaveLength(2);
    act(()=>fake.maps[0].emit('load'));
    expect(screen.getByRole('status').textContent).toContain('Loading street maps');
    fake.maps[1].loaded=true;act(()=>fake.maps[1].emit('load'));
    act(()=>vi.advanceTimersByTime(60_000));
    expect(screen.queryByRole('alert')).toBeNull();
    expect(screen.queryByRole('status')).toBeNull();
  });
  it('reports resource errors even if the remaining map finishes loading',()=>{
    render(<MapView {...props}/>);const map=fake.maps[0];
    act(()=>map.emit('error',{error:Error('tile unavailable')}));
    map.loaded=true;act(()=>map.emit('load'));
    expect(screen.getByRole('alert').textContent).toContain('could not load');
    expect(screen.getByRole('button',{name:'Retry street maps'})).toBeTruthy();
  });
  it('keeps constructor failures recoverable without crashing device controls',()=>{
    fake.failConstruction=true;render(<MapView {...props}/>);
    expect(screen.getByRole('alert').textContent).toContain('could not start');
    fake.failConstruction=false;fireEvent.click(screen.getByRole('button',{name:'Retry street maps'}));
    expect(fake.maps).toHaveLength(1);
  });
  it('avoids passing invalid or infinite positions to the renderer and explains polar display limits',()=>{
    const view=render(<MapView {...props} point={{latitude:90,longitude:0}}/>);
    expect(fake.maps[0].options.center).toEqual([0,85.05112878]);
    expect(screen.getByText(/Your selected coordinates are unchanged/)).toBeTruthy();
    fake.maps[0].easeTo.mockClear();
    view.rerender(<MapView {...props} point={{latitude:NaN,longitude:0}}/>);
    expect(fake.maps[0].easeTo).not.toHaveBeenCalled();
  });
});
