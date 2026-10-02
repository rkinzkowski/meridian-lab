import {initialStatus, type Command, type CommandMap, type DesktopApi, type Library, type Point, type RouteSpec, type RuntimeStatus} from '@meridian/shared-types';
import {distanceMeters, routeDistance, validatePoint} from '@meridian/maps';

const STORE = 'meridian.preview.library.v1';
const emptyLibrary = (): Library => ({places: [], routes: [], history: [], historyEnabled: true});
const copy = <T,>(value: T): T => structuredClone(value);
/** Browser preview only. Native builds always use Rust state and never call this adapter. */
export class BrowserPreviewApi implements DesktopApi {
  readonly native = false;
  private status = initialStatus();
  private library = emptyLibrary();
  private listeners = new Set<(status: RuntimeStatus) => void>();
  private timer: ReturnType<typeof setInterval> | undefined;
  private route: RouteSpec | null = null;
  private lastTick = 0;
  private libraryError: Error | null = null;
  constructor(private readonly storage?: Storage) {
    try {
      const raw = (storage ?? globalThis.localStorage)?.getItem(STORE);
      const record = raw ? JSON.parse(raw) as {version?: number; data?: Library} : null;
      if (raw !== null && raw !== undefined) {
        if (record?.version !== 1 || !record.data || !Array.isArray(record.data.places) || !Array.isArray(record.data.routes) || !Array.isArray(record.data.history)) throw new Error('The saved preview library is invalid.');
        record.data.places.forEach(place => {validatePoint(place.point); if (typeof place.id !== 'string' || !place.id || typeof place.name !== 'string') throw new Error('Invalid saved place.');});
        record.data.routes.forEach(route => this.validateRoute(route));
        record.data.history.forEach(item => {validatePoint(item.point); if (typeof item.id !== 'string' || typeof item.label !== 'string' || !Number.isFinite(item.timestampMs) || !['hardware','demo'].includes(item.mode)) throw new Error('Invalid location history.');});
        this.library = {...record.data, historyEnabled: record.data.historyEnabled !== false};
      }
    } catch {this.libraryError = new Error('The saved preview library could not be read. Its original data has been retained.');}
  }
  private persist(pending: Library): Library {
    try {(this.storage ?? globalThis.localStorage)?.setItem(STORE, JSON.stringify({version: 1, data: pending}));}
    catch {throw new Error('Local storage is unavailable. Your changes could not be saved.');}
    this.library = copy(pending);
    return copy(this.library);
  }
  private changeLibrary(change: (pending: Library) => void): Library {
    if (this.libraryError) throw this.libraryError;
    const pending = copy(this.library); change(pending); return this.persist(pending);
  }
  private publish(): RuntimeStatus {this.listeners.forEach(listener => listener(copy(this.status))); return copy(this.status);}
  async subscribe(listener: (status: RuntimeStatus) => void) {this.listeners.add(listener); return () => {this.listeners.delete(listener);};}
  async subscribeClosePending(_listener: () => void) {return () => {};}
  private stopClock() {if (this.timer) clearInterval(this.timer); this.timer = undefined;}
  private addHistory(point: Point, label: string) {
    if (!this.library.historyEnabled) return;
    this.changeLibrary(pending => {pending.history.unshift({id: crypto.randomUUID(), point, label, timestampMs: Date.now(), mode: 'demo'}); pending.history = pending.history.slice(0, 500);});
  }
  private requireDemo() {
    if (this.status.mode !== 'demo') throw {code: 'DESKTOP_REQUIRED', message: 'Open the desktop application to connect an iPhone.', recovery: 'You can explicitly choose Demo Device to explore this browser preview.', retryable: false};
  }
  private locate(points: Point[], meters: number): Point {
    for (let i = 1; i < points.length; i++) {
      const span = distanceMeters(points[i - 1], points[i]);
      if (meters <= span && span > 0) {
        const a = points[i - 1], b = points[i], rad = Math.PI / 180;
        const lat = a.latitude * rad, lon = a.longitude * rad, endLat = b.latitude * rad, deltaLon = (b.longitude - a.longitude) * rad;
        const bearing = Math.atan2(Math.sin(deltaLon) * Math.cos(endLat), Math.cos(lat) * Math.sin(endLat) - Math.sin(lat) * Math.cos(endLat) * Math.cos(deltaLon));
        const d = meters / 6371008.8;
        const nextLat = Math.asin(Math.max(-1, Math.min(1, Math.sin(lat)*Math.cos(d)+Math.cos(lat)*Math.sin(d)*Math.cos(bearing))));
        const nextLon = lon + Math.atan2(Math.sin(bearing)*Math.sin(d)*Math.cos(lat), Math.cos(d)-Math.sin(lat)*Math.sin(nextLat));
        return {latitude: nextLat/rad, longitude: ((nextLon/rad+540)%360)-180};
      }
      meters -= span;
    }
    return copy(points[points.length - 1]);
  }
  private startClock() {
    this.lastTick = performance.now();
    this.timer = setInterval(() => {
      const progress = this.status.activeRoute;
      if (!this.route || !progress || progress.paused) return;
      const now = performance.now(), elapsed = Math.max(0,Math.min(1,(now - this.lastTick) / 1000));
      this.lastTick = now;
      let distance = progress.distanceMeters + elapsed * progress.speedMps;
      if (distance >= progress.totalMeters) {
        if (progress.loop) distance %= progress.totalMeters;
        else {distance = progress.totalMeters; progress.finished = true; this.status.locationSessionState = 'holding'; this.stopClock();}
      }
      progress.distanceMeters = distance;
      this.status.heldPoint = this.locate(this.route.points, distance);
      this.publish();
    }, 200);
  }
  async call<K extends Command>(command: K, args: CommandMap[K][0]): Promise<CommandMap[K][1]> {
    const a = args as Record<string, unknown>;
    let result: unknown;
    switch (command) {
      case 'runtime_status': result = copy(this.status); break;
      case 'list_devices': result = []; break;
      case 'get_library': if (this.libraryError) throw this.libraryError; result = copy(this.library); break;
      case 'recover_library': {
        if (!this.libraryError) throw new Error('The library does not need recovery.');
        const storage = this.storage ?? globalThis.localStorage;
        const raw = storage?.getItem(STORE);
        if (raw !== null && raw !== undefined) storage?.setItem(`${STORE}.recovered.${crypto.randomUUID()}`,raw);
        result=this.persist(emptyLibrary()); this.libraryError=null; break;
      }
      case 'get_diagnostics': result = {application: 'Meridian Lab', mode: this.status.mode, surface: 'Browser preview', nativeRuntimeAvailable: false, physicalDeviceVerified: false, coordinatesIncluded: false, message: 'Demo actions do not communicate with devices.'}; break;
      case 'save_place': {
        const place = a.place as Library['places'][number]; validatePoint(place.point);
        if (!place.name.trim() || place.name.length > 120) throw new Error('Choose a place name between 1 and 120 characters.');
        if (!place.id || place.id.length > 128) throw new Error('The saved place identifier is invalid.');
        result=this.changeLibrary(pending=>{pending.places=[copy(place),...pending.places.filter(p=>p.id!==place.id)];}); break;
      }
      case 'delete_place': result=this.changeLibrary(pending=>{pending.places=pending.places.filter(p=>p.id!==a.id);}); break;
      case 'save_route': {
        const route = copy(a.route as RouteSpec); this.validateRoute(route);
        result=this.changeLibrary(pending=>{pending.routes=[route,...pending.routes.filter(r=>r.id!==route.id)];}); break;
      }
      case 'delete_route': result=this.changeLibrary(pending=>{pending.routes=pending.routes.filter(r=>r.id!==a.id);}); break;
      case 'clear_history': result=this.changeLibrary(pending=>{pending.history=[];}); break;
      case 'set_history_enabled': result=this.changeLibrary(pending=>{pending.historyEnabled=a.enabled===true;}); break;
      case 'set_demo_mode':
        this.stopClock(); this.route = null;
        this.status = a.enabled ? {...initialStatus(), mode: 'demo', setupStage: 'ready', deviceName: 'Demo Device', developerServicesReady: false, locationSessionState: 'ready', generation: this.status.generation + 1} : {...initialStatus(), generation: this.status.generation + 1};
        result = this.publish(); break;
      default: {
        this.requireDemo();
        if (['teleport','start_route','joystick_step'].includes(command) && this.status.locationSessionState==='closed') throw {code:'SESSION_NOT_READY',message:'The demo session is disconnected.',recovery:'Leave and re-enter Demo Device to reconnect.',retryable:true};
        switch (command) {
          case 'teleport': {
            const point = validatePoint(copy(a.point as Point)); this.stopClock(); this.route = null;
            this.status = {...this.status, heldPoint: point, activeRoute: null, locationSessionState: 'holding', generation: this.status.generation + 1};
            try {this.addHistory(point, 'Demo teleport');}
            catch {this.publish(); throw {code:'HISTORY_SAVE_FAILED',message:'The demo location was applied, but its history entry could not be saved.',recovery:'Check local storage. Restore remains available.',retryable:false};}
            break;
          }
          case 'restore_location': this.stopClock(); this.route = null; this.status = {...this.status, heldPoint: null, activeRoute: null, locationSessionState: this.status.locationSessionState==='closed'?'closed':'ready', generation: this.status.generation + 1}; break;
          case 'disconnect_device': this.stopClock(); this.route = null; this.status = {...this.status, heldPoint:null,activeRoute:null,locationSessionState:'closed', generation: this.status.generation + 1}; break;
          case 'start_route': {
            const route = copy(a.route as RouteSpec); this.validateRoute(route); this.stopClock(); this.route = route;
            this.status = {...this.status, locationSessionState: 'routing', heldPoint: copy(route.points[0]), activeRoute: {id: route.id, name: route.name, distanceMeters: 0, totalMeters: routeDistance(route.points), speedMps: route.speedMps, paused: false, finished: false, loop: route.loop}, generation: this.status.generation + 1};
            this.startClock(); break;
          }
          case 'pause_route': if (!this.status.activeRoute || this.status.activeRoute.finished) throw new Error('There is no running route.'); this.stopClock(); this.status.activeRoute.paused = true; this.status.locationSessionState = 'paused'; break;
          case 'resume_route': if (!this.status.activeRoute?.paused) throw new Error('There is no paused route.'); this.status.activeRoute.paused = false; this.status.locationSessionState = 'routing'; this.startClock(); break;
          case 'set_route_speed': {
            const speed = Number(a.speedMps); if (!Number.isFinite(speed) || speed < 0.1 || speed > 100) throw new Error('Speed must be between 0.1 and 100 m/s.');
            if (!this.status.activeRoute) throw new Error('There is no active route.'); this.status.activeRoute.speedMps = speed; this.lastTick=performance.now(); break;
          }
          case 'joystick_step': {
            if (!this.status.heldPoint) throw new Error('Teleport to a starting point first.');
            const distance = Number(a.distanceMeters), bearing = Number(a.bearingDegrees);
            if (!Number.isFinite(distance) || !Number.isFinite(bearing) || distance < 0 || distance > 10000) throw new Error('Invalid movement.');
            this.stopClock(); this.route = null;
            const rad = Math.PI / 180, d = distance / 6371008.8, lat = this.status.heldPoint.latitude * rad, lon = this.status.heldPoint.longitude * rad, b = bearing * rad;
            const nextLat = Math.asin(Math.sin(lat) * Math.cos(d) + Math.cos(lat) * Math.sin(d) * Math.cos(b));
            const nextLon = lon + Math.atan2(Math.sin(b) * Math.sin(d) * Math.cos(lat), Math.cos(d) - Math.sin(lat) * Math.sin(nextLat));
            this.status = {...this.status, heldPoint: {latitude: nextLat / rad, longitude: ((nextLon / rad + 540) % 360) - 180}, activeRoute: null, locationSessionState: 'holding', generation: this.status.generation + 1}; break;
          }
          default: throw {code: 'DEMO_ONLY', message: 'This device setup action requires the desktop app and a physical iPhone.', recovery: 'Leave Demo Device and open the installed application.', retryable: false};
        }
        result = this.publish();
      }
    }
    return result as CommandMap[K][1];
  }
  private validateRoute(route: RouteSpec) {
    if (typeof route.id !== 'string' || !route.id || route.id.length>128 || typeof route.name !== 'string' || !route.name.trim() || route.name.length > 120 || !Array.isArray(route.points) || route.points.length < 2 || route.points.length > 100000 || typeof route.loop !== 'boolean') throw new Error('A route needs a name and between 2 and 100,000 points.');
    route.points.forEach(validatePoint);
    if (!Number.isFinite(route.speedMps) || route.speedMps < 0.1 || route.speedMps > 100 || routeDistance(route.points) < 0.01) throw new Error('Choose a valid speed and at least two distinct points.');
    if (route.loop && distanceMeters(route.points[0],route.points[route.points.length-1])>1) throw new Error('A repeating route must end within one metre of its start.');
  }
}
