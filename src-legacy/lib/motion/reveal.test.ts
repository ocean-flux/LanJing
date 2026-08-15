import { describe, expect, it } from 'vitest';
import { resolveRevealClasses } from './reveal';

describe('resolveRevealClasses', () => {
  it('keeps visible state motion-safe and reduced-motion safe', () => {
    expect(resolveRevealClasses({ visible: true })).toEqual([
      'opacity-100 translate-y-0',
      'motion-safe:transition-[opacity,transform]',
      'motion-safe:duration-[var(--motion-duration-standard)]',
      'motion-safe:ease-[var(--motion-standard)]',
      'motion-reduce:transition-none',
      'motion-reduce:translate-y-0',
      'motion-reduce:opacity-100',
    ]);
  });

  it('uses opacity and transform only for hidden state', () => {
    expect(resolveRevealClasses({ visible: false })[0]).toBe('opacity-0 translate-y-2');
  });

  it('removes reveal motion when reduced preference is explicit', () => {
    expect(resolveRevealClasses({ visible: false, preference: 'reduced' })).toEqual([
      'opacity-100',
      'translate-y-0',
      'transition-none',
    ]);
  });
});
