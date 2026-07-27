import { describe, expect, it } from 'vitest';
import generatedParityFixture from '../../../../schemas/sources/legado/fixtures/diagnostic-parity.v1.json';
import { SourceAuthoringDocument, type AuthoringDiagnostic } from './source-authoring-document';

interface DiagnosticParityCase {
  readonly name: string;
  readonly scope: 'document' | 'credential_adapter';
  readonly text: string;
  readonly diagnostics: readonly AuthoringDiagnostic[];
}

interface DiagnosticParityFixture {
  readonly fixture_version: 1;
  readonly cases: readonly DiagnosticParityCase[];
}

const parityFixture = generatedParityFixture as unknown as DiagnosticParityFixture;
const documentCases = parityFixture.cases.filter((fixtureCase) => fixtureCase.scope === 'document');

describe('Legado Rust/TypeScript diagnostic parity', () => {
  it('consumes the Rust-owned fixture version without a TypeScript copy', () => {
    expect(parityFixture.fixture_version).toBe(1);
    expect(documentCases.length).toBeGreaterThan(0);
  });

  for (const fixtureCase of documentCases) {
    it(fixtureCase.name, () => {
      const document = SourceAuthoringDocument.open(fixtureCase.text);
      expect(document.diagnostics).toEqual(fixtureCase.diagnostics);
    });
  }
});
