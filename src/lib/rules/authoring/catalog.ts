import generatedCatalog from '../../../../schemas/sources/legado/field-catalog.v1.json';
import { parseJsonPointer } from './json-pointer';

export type KnownSupportClass = 'executable' | 'preserved' | 'blocked';
export type LegadoCatalogFieldType =
  'string' | 'integer' | 'integer_or_string' | 'boolean' | 'object_or_string';

export interface AuthoringLimits {
  readonly max_utf8_bytes: number;
  readonly max_depth: number;
  readonly max_nodes: number;
  readonly max_properties: number;
  readonly max_property_name_utf8_bytes: number;
  readonly max_string_utf8_bytes: number;
}

export type LegadoCatalogField = Readonly<Record<string, unknown>> & {
  readonly pointer: string;
  readonly group: string;
  readonly type: LegadoCatalogFieldType;
  readonly required: boolean;
  readonly hint: string;
  readonly rule_result_type: string | null;
  readonly sensitivity: string;
  readonly support: KnownSupportClass;
  readonly importer_owner: string;
  readonly runtime_owner: string;
};

export interface LegadoFieldCatalog {
  readonly schema_version: 1;
  readonly generator_version: 1;
  readonly limits: AuthoringLimits;
  readonly fields: readonly LegadoCatalogField[];
}

function isPositiveSafeInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value > 0;
}

function loadGeneratedCatalog(value: unknown): LegadoFieldCatalog {
  if (!value || typeof value !== 'object') throw new TypeError('Invalid Legado field catalog');
  const record = value as Record<string, unknown>;
  if (record.schema_version !== 1 || record.generator_version !== 1) {
    throw new TypeError('Unsupported Legado field catalog version');
  }

  if (!record.limits || typeof record.limits !== 'object') {
    throw new TypeError('Legado field catalog limits are missing');
  }
  const limits = record.limits as Record<string, unknown>;
  const limitKeys = [
    'max_utf8_bytes',
    'max_depth',
    'max_nodes',
    'max_properties',
    'max_property_name_utf8_bytes',
    'max_string_utf8_bytes',
  ] as const;
  for (const key of limitKeys) {
    if (!isPositiveSafeInteger(limits[key])) {
      throw new TypeError(`Invalid Legado field catalog limit: ${key}`);
    }
  }

  if (!Array.isArray(record.fields)) throw new TypeError('Legado field catalog fields are missing');
  const pointers = new Set<string>();
  const fields = record.fields.map((candidate) => {
    if (!candidate || typeof candidate !== 'object') {
      throw new TypeError('Invalid Legado field catalog entry');
    }
    const field = candidate as Record<string, unknown>;
    if (typeof field.pointer !== 'string' || !parseJsonPointer(field.pointer).ok) {
      throw new TypeError('Invalid Legado field catalog pointer');
    }
    if (pointers.has(field.pointer)) throw new TypeError('Duplicate Legado field catalog pointer');
    pointers.add(field.pointer);
    if (
      field.support !== 'executable' &&
      field.support !== 'preserved' &&
      field.support !== 'blocked'
    ) {
      throw new TypeError('Invalid Legado field catalog support class');
    }
    if (
      (field.type !== 'string' &&
        field.type !== 'integer' &&
        field.type !== 'integer_or_string' &&
        field.type !== 'boolean' &&
        field.type !== 'object_or_string') ||
      typeof field.required !== 'boolean'
    ) {
      throw new TypeError('Invalid Legado field catalog type contract');
    }
    return Object.freeze(field) as LegadoCatalogField;
  });

  return Object.freeze({
    schema_version: 1,
    generator_version: 1,
    limits: Object.freeze({
      max_utf8_bytes: limits.max_utf8_bytes,
      max_depth: limits.max_depth,
      max_nodes: limits.max_nodes,
      max_properties: limits.max_properties,
      max_property_name_utf8_bytes: limits.max_property_name_utf8_bytes,
      max_string_utf8_bytes: limits.max_string_utf8_bytes,
    }) as AuthoringLimits,
    fields: Object.freeze(fields),
  });
}

/** Rust generator 提交的 catalog 是字段与 limits 的唯一运行时事实源。 */
export const legadoFieldCatalog = loadGeneratedCatalog(generatedCatalog);
export const authoringLimits = legadoFieldCatalog.limits;

const fieldsByPointer = new Map(
  legadoFieldCatalog.fields.map((field) => [field.pointer, field] as const),
);

export function getCatalogField(pointer: string): LegadoCatalogField | undefined {
  return fieldsByPointer.get(pointer);
}
