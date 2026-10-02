import {afterEach,beforeEach,describe,expect,it,vi} from 'vitest';
import {cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {BrowserPreviewApi} from './previewApi';
import {coordinateDraft,sessionReady} from './uiState';
import {initialStatus} from '@meridian/shared-types';
import App from './App';
import {api} from './api';

vi.mock('./api',async()=>({api:new (await import('./previewApi')).BrowserPreviewApi()}));
vi.mock('./MapView',()=>({default:()=> <div>Map rendering is checked separately in a real browser.</div>}));
afterEach(()=>{cleanup();vi.restoreAllMocks();});
beforeEach(async()=>{
  await api.call('set_demo_mode',{enabled:true});
  for(const r of (await api.call('get_library',{})).routes) await api.call('delete_route',{id:r.id});
});

describe('audited desktop controls',()=>{
  it('disables applying an invalid draft without replacing the selected point',async()=>{
    render(<App/>);
    await screen.findByText('DEMO DEVICE — actions in this mode do not change a physical iPhone.');
    fireEvent.change(screen.getByLabelText('Latitude'),{target:{value:'91'}});
    expect((screen.getAllByRole('button',{name:'Teleport'}).at(-1) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByRole('button',{name:'Save place'}).hasAttribute('disabled')).toBe(true);
    expect(screen.getByText(/Enter latitude -90/)).toBeTruthy();
    fireEvent.change(screen.getByLabelText('Latitude'),{target:{value:'40.758'}});
    expect((screen.getAllByRole('button',{name:'Teleport'}).at(-1) as HTMLButtonElement).disabled).toBe(false);
  });
  it('edits a saved route without duplicating its identity or dropping loop/speed',async()=>{
    const route={id:'audit-loop',name:'Audit loop',points:[{latitude:0,longitude:0},{latitude:0,longitude:0.001},{latitude:0,longitude:0}],speedMps:2.7,loop:true};
    await api.call('save_route',{route});
    render(<App/>);
    fireEvent.click(screen.getByRole('button',{name:'Saved'}));
    fireEvent.click(await screen.findByRole('button',{name:'Open route Audit loop'}));
    expect((screen.getByLabelText('Repeat closed route') as HTMLInputElement).checked).toBe(true);
    expect((screen.getByLabelText('Speed') as HTMLSelectElement).value).toBe('2.7');
    fireEvent.change(screen.getByLabelText('Route name'),{target:{value:'Edited loop'}});
    fireEvent.click(screen.getByRole('button',{name:'Save route'}));
    await waitFor(async()=>expect((await api.call('get_library',{})).routes).toEqual([{...route,name:'Edited loop'}]));
  });
});
describe('preview persistence and movement boundaries',()=>{
  it('keeps in-memory saved data unchanged when storage refuses a write',async()=>{
    const memory=new Map<string,string>();
    let fail=false;
    const storage={getItem:(k:string)=>memory.get(k)??null,setItem:(k:string,v:string)=>{if(fail)throw Error('disk full');memory.set(k,v);}} as Storage;
    const preview=new BrowserPreviewApi(storage);
    const place={id:'retained',name:'Retained',point:{latitude:1,longitude:2}};
    await preview.call('save_place',{place});fail=true;
    await expect(preview.call('delete_place',{id:place.id})).rejects.toThrow('could not be saved');
    expect((await preview.call('get_library',{})).places).toEqual([place]);
  });
  it('preserves corrupt saved bytes before explicitly recovering',async()=>{
    const memory=new Map([['meridian.preview.library.v1','not json']]);
    const storage={getItem:(k:string)=>memory.get(k)??null,setItem:(k:string,v:string)=>{memory.set(k,v);}} as Storage;
    const preview=new BrowserPreviewApi(storage);
    await expect(preview.call('get_library',{})).rejects.toThrow();
    const result=await preview.call('recover_library',{});
    expect(result.places).toEqual([]);
    expect([...memory.entries()].some(([key,value])=>key.includes('.recovered.')&&value==='not json')).toBe(true);
  });
  it('never considers a failed restore or disconnected demo ready',()=>{
    for(const locationSessionState of ['closed','recovering','restoreFailed'] as const) expect(sessionReady({...initialStatus(),mode:'demo',locationSessionState})).toBe(false);
    expect(sessionReady({...initialStatus(),mode:'demo',locationSessionState:'ready'})).toBe(true);
    expect(coordinateDraft('','0')).toBeNull();
    expect(coordinateDraft('NaN','0')).toBeNull();
    expect(coordinateDraft('90','-180')).toEqual({latitude:90,longitude:-180});
  });
});
describe('pending restoration close guard',()=>{
  function emitClosePending(){
    vi.spyOn(api,'subscribeClosePending').mockImplementation(async listener=>{listener();return ()=>{};});
  }
  it('dismisses the pending close dialog without invoking native close',async()=>{
    emitClosePending();
    const close=vi.spyOn(api,'call');
    render(<App/>);
    const dialog=await screen.findByRole('alertdialog',{name:'Restoration is still pending'});
    expect(dialog).toBeTruthy();
    fireEvent.click(screen.getByRole('button',{name:'Keep open'}));
    expect(screen.queryByRole('alertdialog')).toBeNull();
    expect(close).not.toHaveBeenCalledWith('close_with_pending_restore',{});
  });
  it('invokes native close only after explicit confirmation',async()=>{
    emitClosePending();
    const close=vi.spyOn(api,'call');
    render(<App/>);
    await screen.findByRole('alertdialog',{name:'Restoration is still pending'});
    close.mockResolvedValue(undefined as never);
    fireEvent.click(screen.getByRole('button',{name:'Close with restore pending'}));
    await waitFor(()=>expect(close).toHaveBeenCalledWith('close_with_pending_restore',{}));
  });
  it('keeps the modal open and exposes a retry after journal persistence fails',async()=>{
    emitClosePending();
    const close=vi.spyOn(api,'call');
    render(<App/>);
    await screen.findByRole('alertdialog',{name:'Restoration is still pending'});
    close.mockRejectedValue({code:'RESTORE_JOURNAL_WRITE_FAILED',message:'The restore reminder could not be saved.',recovery:'Check storage and retry.',retryable:true} as never);
    fireEvent.click(screen.getByRole('button',{name:'Close with restore pending'}));
    expect(await screen.findByText('The restore reminder could not be saved.')).toBeTruthy();
    expect(screen.getByRole('alertdialog',{name:'Restoration is still pending'})).toBeTruthy();
    close.mockResolvedValue(undefined as never);
    fireEvent.click(screen.getByRole('button',{name:'Close with restore pending'}));
    await waitFor(()=>expect(close).toHaveBeenCalledWith('close_with_pending_restore',{}));
  });
});
