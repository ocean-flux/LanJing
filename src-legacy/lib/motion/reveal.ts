export type MotionPreference = 'normal' | 'reduced';

export type RevealClassInput = {
  visible?: boolean;
  preference?: MotionPreference;
};

export function resolveRevealClasses({
  visible = true,
  preference = 'normal',
}: RevealClassInput = {}): string[] {
  const stateClass = visible ? 'opacity-100 translate-y-0' : 'opacity-0 translate-y-2';

  if (preference === 'reduced') {
    return ['opacity-100', 'translate-y-0', 'transition-none'];
  }

  return [
    stateClass,
    'motion-safe:transition-[opacity,transform]',
    'motion-safe:duration-[var(--motion-duration-standard)]',
    'motion-safe:ease-[var(--motion-standard)]',
    'motion-reduce:transition-none',
    'motion-reduce:translate-y-0',
    'motion-reduce:opacity-100',
  ];
}
