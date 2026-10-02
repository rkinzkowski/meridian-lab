import type {Point} from '@meridian/shared-types';
export interface SearchResult {id: string; name: string; detail: string; point: Point}
export interface GeocodingProvider {
  readonly name: string;
  search(query: string, signal?: AbortSignal): Promise<SearchResult[]>;
  reverseGeocode(point: Point, signal?: AbortSignal): Promise<SearchResult>;
}
export interface MapProvider {id: string; name: string; online: boolean; styleUrl?: string}
export const mapProviders: MapProvider[] = [
  {id: 'offline', name: 'Offline atlas', online: false},
  {id: 'dark', name: 'Dark streets', online: true, styleUrl: 'https://tiles.openfreemap.org/styles/dark'},
  {id: 'light', name: 'Light streets', online: true, styleUrl: 'https://tiles.openfreemap.org/styles/positron'},
  {id: 'streets', name: 'Detailed streets', online: true, styleUrl: 'https://tiles.openfreemap.org/styles/liberty'},
];
export function validatePoint(point: Point): Point {
  if (!Number.isFinite(point.latitude) || !Number.isFinite(point.longitude) || Math.abs(point.latitude) > 90 || Math.abs(point.longitude) > 180) throw new Error('Enter a latitude from −90 to 90 and a longitude from −180 to 180.');
  return point;
}
export function parseCoordinates(value: string): Point | null {
  const match = value.trim().match(/^([+-]?(?:\d+(?:\.\d*)?|\.\d+))\s*[,;]\s*([+-]?(?:\d+(?:\.\d*)?|\.\d+))$/);
  if (!match) return null;
  return validatePoint({latitude: Number(match[1]), longitude: Number(match[2])});
}
export const formatPoint = (point: Point): string => `${point.latitude.toFixed(5)}, ${point.longitude.toFixed(5)}`;
export const qaPresets:SearchResult[] = [
  {id:'null-island',name:'Null Island',detail:'Equator / prime meridian',point:{latitude:0,longitude:0}},
  {id:'north-pole',name:'Near North Pole',detail:'Polar proximity',point:{latitude:89.999,longitude:0}},
  {id:'south-pole',name:'Near South Pole',detail:'Polar proximity',point:{latitude:-89.999,longitude:0}},
  {id:'dateline-east',name:'Date Line east',detail:'Longitude boundary',point:{latitude:0,longitude:179.999}},
  {id:'dateline-west',name:'Date Line west',detail:'Longitude boundary',point:{latitude:0,longitude:-179.999}},
  {id:'times-square',name:'Times Square',detail:'New York test fixture',point:{latitude:40.758,longitude:-73.9855}},
];
export function distanceMeters(a: Point, b: Point): number {
  const rad = Math.PI / 180, dLat = (b.latitude - a.latitude) * rad, dLon = (b.longitude - a.longitude) * rad;
  const h = Math.sin(dLat / 2) ** 2 + Math.cos(a.latitude * rad) * Math.cos(b.latitude * rad) * Math.sin(dLon / 2) ** 2;
  return 6371008.8 * 2 * Math.atan2(Math.sqrt(h), Math.sqrt(Math.max(0, 1 - h)));
}
export const routeDistance = (points: Point[]): number => points.slice(1).reduce((sum, p, i) => sum + distanceMeters(points[i], p), 0);
const landmarks: SearchResult[] = [
  {id: 'new-york', name: 'New York', detail: 'United States · offline city', point: {latitude: 40.7128, longitude: -74.006}},
  {id: 'central-park', name: 'Central Park', detail: 'New York · offline landmark', point: {latitude: 40.7829, longitude: -73.9654}},
  {id: 'san-francisco', name: 'San Francisco', detail: 'United States · offline city', point: {latitude: 37.7749, longitude: -122.4194}},
  {id: 'london', name: 'London', detail: 'United Kingdom · offline city', point: {latitude: 51.5074, longitude: -0.1278}},
  {id: 'paris', name: 'Paris', detail: 'France · offline city', point: {latitude: 48.8566, longitude: 2.3522}},
  {id: 'tokyo', name: 'Tokyo', detail: 'Japan · offline city', point: {latitude: 35.6762, longitude: 139.6503}},
  {id: 'sydney', name: 'Sydney', detail: 'Australia · offline city', point: {latitude: -33.8688, longitude: 151.2093}},
  {id: 'berlin', name: 'Berlin', detail: 'Germany · offline city', point: {latitude: 52.52, longitude: 13.405}},
  {id: 'singapore', name: 'Singapore', detail: 'Singapore · offline city', point: {latitude: 1.3521, longitude: 103.8198}},
  {id: 'cape-town', name: 'Cape Town', detail: 'South Africa · offline city', point: {latitude: -33.9249, longitude: 18.4241}},
];
export const offlineGeocoder: GeocodingProvider = {
  name: 'Offline landmarks',
  async search(query) {
    const point = parseCoordinates(query);
    if (point) return [{id: 'coordinate', name: 'Coordinates', detail: formatPoint(point), point}];
    const terms = query.trim().toLowerCase();
    return terms ? landmarks.filter(p => `${p.name} ${p.detail}`.toLowerCase().includes(terms)) : landmarks.slice(0, 4);
  },
  async reverseGeocode(point) {validatePoint(point); return {id: 'coordinate', name: 'Selected location', detail: formatPoint(point), point};},
};
/** For licensed/self-hosted Nominatim-compatible endpoints, never silently sends data to a public provider. */
export class NominatimProvider implements GeocodingProvider {
  readonly name = 'Configured geocoder';
  constructor(private readonly endpoint: string, private readonly request: typeof fetch = fetch) {
    const url = new URL(endpoint);
    if (url.protocol !== 'https:' || url.username || url.password) throw new Error('Geocoding requires an HTTPS endpoint without credentials.');
  }
  private async get(path: string, params: Record<string, string>, signal?: AbortSignal): Promise<unknown> {
    const url = new URL(path, this.endpoint.endsWith('/') ? this.endpoint : `${this.endpoint}/`);
    Object.entries({...params, format: 'jsonv2'}).forEach(([k, v]) => url.searchParams.set(k, v));
    const response = await this.request(url, {signal, credentials: 'omit', referrerPolicy: 'no-referrer'});
    if (!response.ok) throw new Error('The search provider is temporarily unavailable.');
    return response.json();
  }
  private result(value: unknown): SearchResult {
    if (!value || typeof value !== 'object') throw new Error('Invalid geocoding response.');
    const r = value as Record<string, unknown>;
    if (typeof r.display_name !== 'string' || !['string', 'number'].includes(typeof r.lat) || !['string', 'number'].includes(typeof r.lon)) throw new Error('Invalid geocoding response.');
    const point = validatePoint({latitude: Number(r.lat), longitude: Number(r.lon)});
    return {id: String(r.place_id ?? formatPoint(point)), name: r.display_name.split(',')[0], detail: r.display_name, point};
  }
  async search(query: string, signal?: AbortSignal) {
    const point = parseCoordinates(query);
    if (point) return offlineGeocoder.search(query);
    const result = await this.get('search', {q: query, limit: '6'}, signal);
    if (!Array.isArray(result)) throw new Error('Invalid geocoding response.');
    return result.map(r => this.result(r));
  }
  async reverseGeocode(point: Point, signal?: AbortSignal) {
    validatePoint(point);
    return this.result(await this.get('reverse', {lat: String(point.latitude), lon: String(point.longitude)}, signal));
  }
}
