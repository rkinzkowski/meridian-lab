import {describe,it,expect} from 'vitest';
import {exportGpx,parseGpx} from './gpx';
import {parseCoordinates,distanceMeters} from '@meridian/maps';
import {BrowserPreviewApi} from './previewApi';

describe('coordinates and GPX',()=>{
  it('accepts boundaries and rejects nonfinite/range input',()=>{expect(parseCoordinates('90, -180')).toEqual({latitude:90,longitude:-180});expect(()=>parseCoordinates('91, 0')).toThrow();expect(parseCoordinates('NaN,0')).toBeNull();});
  it('uses the short path across the date line',()=>expect(distanceMeters({latitude:0,longitude:179.999},{latitude:0,longitude:-179.999})).toBeCloseTo(222.39,1));
  it('roundtrips text and points without injecting XML',()=>{const route={id:'test',name:'A < B & C',points:[{latitude:0,longitude:0},{latitude:1,longitude:1}],speedMps:1,loop:false};expect(parseGpx(exportGpx(route))).toEqual({name:route.name,points:route.points});});
  it('rejects external entities and disconnected tracks',()=>{expect(()=>parseGpx('<!DOCTYPE gpx [<!ENTITY x SYSTEM "file:///secret">]><gpx/>')).toThrow();expect(()=>parseGpx('<gpx><trk><trkseg/><trkseg/></trk></gpx>')).toThrow();});
});
describe('explicit browser demo boundary',()=>{
  it('never silently uses a fake device',async()=>{const api=new BrowserPreviewApi();await expect(api.call('teleport',{point:{latitude:1,longitude:2}})).rejects.toMatchObject({code:'DESKTOP_REQUIRED'});expect((await api.call('runtime_status',{})).physicalDeviceVerified).toBe(false);});
  it('clear retires demo state and leaving demo prevents further movement',async()=>{const api=new BrowserPreviewApi();await api.call('set_demo_mode',{enabled:true});await api.call('teleport',{point:{latitude:1,longitude:2}});expect((await api.call('restore_location',{})).heldPoint).toBeNull();await api.call('set_demo_mode',{enabled:false});await expect(api.call('teleport',{point:{latitude:3,longitude:4}})).rejects.toMatchObject({code:'DESKTOP_REQUIRED'});});
});
