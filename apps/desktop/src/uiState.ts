import type {Point, RuntimeStatus} from '@meridian/shared-types';

export function coordinateDraft(latitude: string, longitude: string): Point | null {
  if (!latitude.trim() || !longitude.trim()) return null;
  const point = {latitude: Number(latitude), longitude: Number(longitude)};
  return Number.isFinite(point.latitude) && Number.isFinite(point.longitude)
    && Math.abs(point.latitude) <= 90 && Math.abs(point.longitude) <= 180 ? point : null;
}

export function sessionReady(status: RuntimeStatus): boolean {
  return !status.recovering && ['ready', 'holding', 'routing', 'paused'].includes(status.locationSessionState)
    && (status.developerServicesReady || status.mode === 'demo');
}

export function validSpeed(speed: number): boolean {
  return Number.isFinite(speed) && speed >= 0.1 && speed <= 100;
}
