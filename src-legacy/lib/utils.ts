import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

export function cn(...values: ClassValue[]) {
  return twMerge(clsx(values));
}

// oxlint-disable-next-line typescript/no-explicit-any -- 保留 Svelte 组件 child 类型的任意性。
export type WithoutChild<T> = T extends { child?: any } ? Omit<T, 'child'> : T;
// oxlint-disable-next-line typescript/no-explicit-any -- 保留 Svelte 组件 children 类型的任意性。
export type WithoutChildren<T> = T extends { children?: any } ? Omit<T, 'children'> : T;
export type WithoutChildrenOrChild<T> = WithoutChildren<WithoutChild<T>>;
export type WithElementRef<T, U extends HTMLElement = HTMLElement> = T & { ref?: U | null };
