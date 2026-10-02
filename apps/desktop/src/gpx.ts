import {validatePoint} from '@meridian/maps';
import type {Point, RouteSpec} from '@meridian/shared-types';

export const MAX_GPX_BYTES = 8 * 1024 * 1024;
export function parseGpx(xml: string): {name: string; points: Point[]} {
  if (new TextEncoder().encode(xml).length > MAX_GPX_BYTES) throw new Error('Choose a GPX file smaller than 8 MB.');
  if (/<!DOCTYPE|<!ENTITY/i.test(xml)) throw new Error('GPX files with external entities are not supported.');
  const doc = new DOMParser().parseFromString(xml, 'application/xml');
  if (doc.querySelector('parsererror') || doc.documentElement.localName !== 'gpx') throw new Error('This file is not valid GPX.');
  const segments = Array.from(doc.getElementsByTagNameNS('*', 'trkseg'));
  if (segments.length > 1) throw new Error('This GPX has separate track segments. Import a single continuous segment to avoid joining gaps.');
  let nodes = Array.from(doc.getElementsByTagNameNS('*', 'trkpt'));
  if (!nodes.length) nodes = Array.from(doc.getElementsByTagNameNS('*', 'rtept'));
  if (nodes.length < 2 || nodes.length > 100000) throw new Error('A GPX route must contain between 2 and 100,000 points.');
  const points = nodes.map(node => {
    const lat = node.getAttribute('lat'), lon = node.getAttribute('lon');
    if (lat === null || lon === null || !lat.trim() || !lon.trim()) throw new Error('A GPX point is missing latitude or longitude.');
    return validatePoint({latitude: Number(lat), longitude: Number(lon)});
  });
  const name = (doc.getElementsByTagNameNS('*', 'name')[0]?.textContent?.trim() || 'Imported route').slice(0, 120);
  return {name, points};
}
const escapeXml = (value: string) => value.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;').replace(/'/g, '&apos;');
export function exportGpx(route: RouteSpec): string {
  return `<?xml version="1.0" encoding="UTF-8"?>\n<gpx version="1.1" creator="Meridian Lab" xmlns="http://www.topografix.com/GPX/1/1"><trk><name>${escapeXml(route.name)}</name><trkseg>${route.points.map(point => {validatePoint(point); return `<trkpt lat="${point.latitude}" lon="${point.longitude}"/>`;}).join('')}</trkseg></trk></gpx>`;
}
export function downloadText(filename: string, data: string, type = 'application/json') {
  const url = URL.createObjectURL(new Blob([data], {type}));
  const link = document.createElement('a'); link.href = url; link.download = filename; document.body.append(link); link.click(); link.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
