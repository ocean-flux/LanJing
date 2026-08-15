// paraglide 服务端中间件：仅在 SSR 启用时生效
// 当前项目为 SPA 模式（src/routes/+layout.ts 中 ssr=false），此文件不会执行，
// 但保留标准 paraglide 服务端配置，便于将来启用 SSR 时直接生效。
import type { Handle } from '@sveltejs/kit';
import { paraglideMiddleware } from '$lib/paraglide/server.js';
import { getTextDirection } from '$lib/paraglide/runtime.js';

const paraglideHandle: Handle = ({ event, resolve }) =>
  paraglideMiddleware(event.request, ({ request: localizedRequest, locale }) => {
    event.request = localizedRequest;
    return resolve(event, {
      transformPageChunk: ({ html }) =>
        html.replace('%lang%', locale).replace('%dir%', getTextDirection(locale)),
    });
  });

export const handle: Handle = paraglideHandle;
