import { render, screen } from '@testing-library/svelte';
import type { ComponentProps } from 'svelte';
import { describe, expect, expectTypeOf, it } from 'vitest';
import Input from './input.svelte';

type InputProps = ComponentProps<typeof Input>;
type FileInputProps = Extract<InputProps, { type: 'file' }>;
type FileInputRejectsValue = { type: 'file'; value: string } extends InputProps ? false : true;
const fileInputRejectsValue: FileInputRejectsValue = true;

describe('Input', () => {
  it('excludes value from file input props', () => {
    expectTypeOf<FileInputProps['value']>().toEqualTypeOf<undefined>();
    expect(fileInputRejectsValue).toBe(true);
  });

  it('does not write a supplied runtime value into a file input', () => {
    expect(() =>
      render(Input, {
        type: 'file',
        value: 'forbidden-file-value',
        'aria-label': 'Upload file',
      } as unknown as InputProps),
    ).not.toThrow();

    const input = screen.getByLabelText('Upload file') as HTMLInputElement;
    expect(input.type).toBe('file');
    expect(input.value).toBe('');
  });

  it('keeps the normal value binding path', () => {
    render(Input, { type: 'text', value: 'search term', 'aria-label': 'Search' });

    expect((screen.getByLabelText('Search') as HTMLInputElement).value).toBe('search term');
  });
});
