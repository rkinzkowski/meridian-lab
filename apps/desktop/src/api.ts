import {invoke, isTauri} from '@tauri-apps/api/core';
import {listen} from '@tauri-apps/api/event';
import type {Command, CommandMap, DesktopApi, RuntimeStatus} from '@meridian/shared-types';
import {BrowserPreviewApi} from './previewApi';

class NativeApi implements DesktopApi {
  readonly native = true;
  call<K extends Command>(command: K, args: CommandMap[K][0]): Promise<CommandMap[K][1]> {return invoke<CommandMap[K][1]>(command, args);}
  async subscribe(listener: (status: RuntimeStatus) => void): Promise<() => void> {
    return listen<RuntimeStatus>('runtime-status', event => listener(event.payload));
  }
  async subscribeClosePending(listener: () => void): Promise<() => void> {
    return listen<Record<string, never>>('close-restore-pending', () => listener());
  }
}
export const api: DesktopApi = isTauri() ? new NativeApi() : new BrowserPreviewApi();
