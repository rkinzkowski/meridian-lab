export type Point = {latitude: number; longitude: number};
export type TypedError = {code: string; message: string; recovery: string; retryable: boolean};
export type Transport = 'usb' | 'wifi' | 'rsd';
export type Device = {id: string; name: string; productVersion: string; connectionType: string};
export type RouteSpec = {id: string; name: string; points: Point[]; speedMps: number; loop: boolean};
export type RouteProgress = {id: string; name: string; distanceMeters: number; totalMeters: number; speedMps: number; paused: boolean; finished: boolean; loop: boolean};
export type RuntimeStatus = {
  platform: string; mode: 'hardware' | 'demo'; setupStage: 'requirements' | 'connect' | 'developer' | 'ready';
  deviceFound: boolean; deviceName: string | null; productVersion: string | null; deviceIdRedacted: string | null;
  usbCablePresent: boolean; lockdownTrusted: boolean; developerModeState: 'unknown' | 'enabled' | 'disabled';
  developerImageState: 'unknown' | 'mounted'; developerServicesReady: boolean; remotePairingState: 'unknown' | 'verified';
  activeTransport: Transport | null; tunnelState: 'closed' | 'starting' | 'ready' | 'failed'; rsdState: 'closed' | 'ready' | 'failed';
  locationSessionState: 'closed' | 'opening' | 'ready' | 'holding' | 'routing' | 'paused' | 'recovering' | 'restoreFailed';
  heldPoint: Point | null; activeRoute: RouteProgress | null; recovering: boolean; generation: number;
  lastTypedError: TypedError | null; physicalDeviceVerified: boolean; usbmuxReachable: boolean; busy: boolean;
};
export type SavedPlace = {id: string; name: string; point: Point};
export type HistoryEntry = {id: string; label: string; point: Point; timestampMs: number; mode: 'hardware' | 'demo'};
export type Library = {places: SavedPlace[]; routes: RouteSpec[]; history: HistoryEntry[]; historyEnabled: boolean};
export type ConnectRequest = {udid: string; transport: Transport; rsdAddress?: string; rsdPort?: number};
export type CommandMap = {
  runtime_status: [{}, RuntimeStatus]; list_devices: [{}, Device[]]; connect_device: [{request: ConnectRequest}, RuntimeStatus];
  trust_device: [{udid: string}, RuntimeStatus]; prepare_device: [{udid: string}, RuntimeStatus]; disconnect_device: [{}, RuntimeStatus];
  teleport: [{point: Point}, RuntimeStatus]; restore_location: [{}, RuntimeStatus]; start_route: [{route: RouteSpec}, RuntimeStatus];
  recover_session: [{}, RuntimeStatus];
  pause_route: [{}, RuntimeStatus]; resume_route: [{}, RuntimeStatus]; set_route_speed: [{speedMps: number}, RuntimeStatus];
  joystick_step: [{bearingDegrees: number; distanceMeters: number}, RuntimeStatus]; prepare_wireless: [{udid: string}, RuntimeStatus];
  get_diagnostics: [{}, Record<string, unknown>]; set_demo_mode: [{enabled: boolean}, RuntimeStatus]; get_library: [{}, Library];
  recover_library: [{}, Library];
  save_place: [{place: SavedPlace}, Library]; delete_place: [{id: string}, Library]; clear_history: [{}, Library];
  save_route: [{route: RouteSpec}, Library]; delete_route: [{id: string}, Library]; set_history_enabled: [{enabled: boolean}, Library];
  close_with_pending_restore: [{}, void];
};
export type Command = keyof CommandMap;
export interface DesktopApi {
  readonly native: boolean;
  call<K extends Command>(command: K, args: CommandMap[K][0]): Promise<CommandMap[K][1]>;
  subscribe(listener: (status: RuntimeStatus) => void): Promise<() => void>;
  subscribeClosePending(listener: () => void): Promise<() => void>;
}
export function initialStatus(): RuntimeStatus {
  return {platform: 'browser preview', mode: 'hardware', setupStage: 'requirements', deviceFound: false, deviceName: null, productVersion: null, deviceIdRedacted: null, usbCablePresent: false, lockdownTrusted: false, developerModeState: 'unknown', developerImageState: 'unknown', developerServicesReady: false, remotePairingState: 'unknown', activeTransport: null, tunnelState: 'closed', rsdState: 'closed', locationSessionState: 'closed', heldPoint: null, activeRoute: null, recovering: false, generation: 0, lastTypedError: null, physicalDeviceVerified: false, usbmuxReachable: false, busy: false};
}
export function errorFrom(value: unknown): TypedError {
  if (value && typeof value === 'object' && 'code' in value && 'message' in value) {
    const item = value as Partial<TypedError>;
    return {code: String(item.code), message: String(item.message), recovery: String(item.recovery ?? 'Open diagnostics for more details.'), retryable: item.retryable === true};
  }
  return {code: 'ACTION_FAILED', message: value instanceof Error ? value.message : 'The action could not be completed.', recovery: 'Check the connection and try again.', retryable: false};
}
