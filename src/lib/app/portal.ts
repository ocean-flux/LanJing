/**
 * 将节点挂到 document.body，避开祖先 overflow / filter 裁切。
 * Svelte action：`use:portal`
 */
export function portal(node: HTMLElement) {
  if (typeof document === 'undefined') {
    return {};
  }

  const host = document.body;
  host.appendChild(node);

  return {
    destroy() {
      if (node.parentNode === host) {
        host.removeChild(node);
      }
    },
  };
}
