export type RuntimePlatform = 'windows' | 'macos' | 'linux' | 'ios' | 'android' | 'unknown';

function mapPlatform(platform: string): RuntimePlatform {
  switch (platform) {
    case 'windows':
    case 'macos':
    case 'linux':
    case 'ios':
    case 'android':
      return platform;
    default:
      return 'unknown';
  }
}

export async function resolveRuntimePlatform(): Promise<RuntimePlatform> {
  if (typeof window === 'undefined' || !('__TAURI_INTERNALS__' in window)) return 'unknown';

  try {
    // The OS plugin is Tauri-only; a static import would put it in the browser/SSR path.
    const { platform } = await import('@tauri-apps/plugin-os');
    return mapPlatform(platform());
  } catch {
    return 'unknown';
  }
}
