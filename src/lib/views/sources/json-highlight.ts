/** Lightweight JSON tokenizer for install-surface highlight (no Monaco). */

export type JsonTokenKind =
  'key' | 'string' | 'number' | 'boolean' | 'null' | 'punct' | 'ws' | 'plain';

export interface JsonToken {
  kind: JsonTokenKind;
  text: string;
}

type Frame =
  | { type: 'object'; expect: 'key' | 'colon' | 'value' | 'comma' }
  | { type: 'array'; expect: 'value' | 'comma' };

function isWs(ch: string): boolean {
  return ch === ' ' || ch === '\t' || ch === '\n' || ch === '\r' || ch === '\f';
}

function isDigit(ch: string): boolean {
  return ch >= '0' && ch <= '9';
}

function readString(input: string, start: number): { end: number; text: string } {
  let i = start + 1;
  while (i < input.length) {
    const ch = input[i]!;
    if (ch === '\\') {
      i += 2;
      continue;
    }
    if (ch === '"') {
      return { end: i + 1, text: input.slice(start, i + 1) };
    }
    i += 1;
  }
  return { end: input.length, text: input.slice(start) };
}

function readNumber(input: string, start: number): { end: number; text: string } {
  let i = start;
  if (input[i] === '-') i += 1;
  while (i < input.length && isDigit(input[i]!)) i += 1;
  if (input[i] === '.') {
    i += 1;
    while (i < input.length && isDigit(input[i]!)) i += 1;
  }
  if (input[i] === 'e' || input[i] === 'E') {
    i += 1;
    if (input[i] === '+' || input[i] === '-') i += 1;
    while (i < input.length && isDigit(input[i]!)) i += 1;
  }
  return { end: i, text: input.slice(start, i) };
}

function readIdent(input: string, start: number): { end: number; text: string } {
  let i = start;
  while (i < input.length) {
    const ch = input[i]!;
    if ((ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z') || ch === '_' || isDigit(ch)) {
      i += 1;
      continue;
    }
    break;
  }
  return { end: i, text: input.slice(start, i) };
}

function skipWs(input: string, start: number): { end: number; text: string } {
  let i = start;
  while (i < input.length && isWs(input[i]!)) i += 1;
  return { end: i, text: input.slice(start, i) };
}

function afterValue(stack: Frame[]): void {
  const top = stack[stack.length - 1];
  if (!top) return;
  if (top.type === 'object') top.expect = 'comma';
  else top.expect = 'comma';
}

/**
 * Tokenize JSON-ish text. Never throws — broken input still yields tokens.
 * Distinguishes object keys from string values via a lightweight stack.
 */
export function tokenizeJson(input: string): JsonToken[] {
  const tokens: JsonToken[] = [];
  const stack: Frame[] = [];
  let i = 0;

  while (i < input.length) {
    const ch = input[i]!;

    if (isWs(ch)) {
      const ws = skipWs(input, i);
      tokens.push({ kind: 'ws', text: ws.text });
      i = ws.end;
      continue;
    }

    if (ch === '"') {
      const str = readString(input, i);
      const top = stack[stack.length - 1];
      const asKey = top?.type === 'object' && top.expect === 'key';
      tokens.push({ kind: asKey ? 'key' : 'string', text: str.text });
      i = str.end;
      if (asKey && top) {
        top.expect = 'colon';
      } else {
        afterValue(stack);
      }
      continue;
    }

    if (ch === '-' || isDigit(ch)) {
      const num = readNumber(input, i);
      if (num.text.length > 0 && !(num.text === '-')) {
        tokens.push({ kind: 'number', text: num.text });
        i = num.end;
        afterValue(stack);
        continue;
      }
    }

    if ((ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z') || ch === '_') {
      const ident = readIdent(input, i);
      if (ident.text === 'true' || ident.text === 'false') {
        tokens.push({ kind: 'boolean', text: ident.text });
      } else if (ident.text === 'null') {
        tokens.push({ kind: 'null', text: ident.text });
      } else {
        tokens.push({ kind: 'plain', text: ident.text });
      }
      i = ident.end;
      afterValue(stack);
      continue;
    }

    if (ch === '{' || ch === '}' || ch === '[' || ch === ']' || ch === ':' || ch === ',') {
      tokens.push({ kind: 'punct', text: ch });
      if (ch === '{') {
        stack.push({ type: 'object', expect: 'key' });
      } else if (ch === '[') {
        stack.push({ type: 'array', expect: 'value' });
      } else if (ch === '}' || ch === ']') {
        stack.pop();
        afterValue(stack);
      } else if (ch === ':') {
        const top = stack[stack.length - 1];
        if (top?.type === 'object') top.expect = 'value';
      } else if (ch === ',') {
        const top = stack[stack.length - 1];
        if (top?.type === 'object') top.expect = 'key';
        else if (top?.type === 'array') top.expect = 'value';
      }
      i += 1;
      continue;
    }

    tokens.push({ kind: 'plain', text: ch });
    i += 1;
  }

  return tokens;
}

function escapeHtml(text: string): string {
  return text
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;');
}

const KIND_CLASS: Record<JsonTokenKind, string> = {
  key: 'jh-key',
  string: 'jh-string',
  number: 'jh-number',
  boolean: 'jh-bool',
  null: 'jh-null',
  punct: 'jh-punct',
  ws: 'jh-ws',
  plain: 'jh-plain',
};

/** Render tokens to safe HTML spans for the mirror layer. */
export function tokensToHtml(tokens: JsonToken[]): string {
  let html = '';
  for (const token of tokens) {
    if (token.kind === 'ws') {
      html += escapeHtml(token.text);
      continue;
    }
    html += `<span class="${KIND_CLASS[token.kind]}">${escapeHtml(token.text)}</span>`;
  }
  return html;
}

/** Full pipeline: source → safe highlighted HTML. Never throws. */
export function highlightJsonHtml(input: string): string {
  try {
    return tokensToHtml(tokenizeJson(input));
  } catch {
    return escapeHtml(input);
  }
}
