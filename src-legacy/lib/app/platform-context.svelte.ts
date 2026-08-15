import type { RuntimePlatform } from './platform-runtime';
import { getContext, setContext } from 'svelte';

export type PlatformContext = {
  readonly platform: RuntimePlatform;
};

const PLATFORM_CONTEXT = Symbol('lanjing-platform-context');
const UNKNOWN_PLATFORM_CONTEXT: PlatformContext = Object.freeze({ platform: 'unknown' });

export function getPlatformContext(): PlatformContext {
  return getContext<PlatformContext | undefined>(PLATFORM_CONTEXT) ?? UNKNOWN_PLATFORM_CONTEXT;
}

export function setPlatformContext(context: PlatformContext): PlatformContext {
  return setContext(PLATFORM_CONTEXT, context);
}
