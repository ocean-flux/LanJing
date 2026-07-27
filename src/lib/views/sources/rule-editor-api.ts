import { invoke } from '@tauri-apps/api/core';
import type {
  CapabilityGrantPreset,
  InstallCandidate,
  InstalledSource,
} from '$lib/stores/rules.svelte';

export type SourceDocumentId = string;
export type CredentialSlotId = string;
export type SourceDocumentFormat = 'legado' | 'maccms10_endpoint';
export type SourceDocumentState = 'draft' | 'linked';

export interface SourceDocumentSummary {
  document_id: SourceDocumentId;
  format: SourceDocumentFormat;
  title: string;
  state: SourceDocumentState;
  source_identity: string | null;
  revision: number;
  installed_revision: number | null;
  masked_hash: string;
  credential_slot_count: number;
  schema_version: number;
  created_at_ms: number;
  updated_at_ms: number;
}

export interface CredentialSlotSummary {
  slot_id: CredentialSlotId;
  path: string;
  name: string;
  has_value: boolean;
}

export interface MaskedSourceDocument {
  summary: SourceDocumentSummary;
  masked_text: string;
  credential_slots: CredentialSlotSummary[];
}

export interface DocumentRef {
  document_id: SourceDocumentId;
  document_revision: number;
}

export interface SourceDocumentRevisionPin {
  pin_id: string;
  document_id: SourceDocumentId;
  document_revision: number;
  expires_at_ms: number;
}

export type PinSourceDocumentRevisionRequest = DocumentRef;

export interface ReleaseSourceDocumentRevisionPinRequest {
  pin_id: string;
}

export interface SourceDocumentRevisionPinReleased {
  status: 'released';
  pin_id: string;
}

export type SourceDocumentRebaseMode = { kind: 'merge' } | { kind: 'fork'; title: string };

export type SourceDocumentCredentialResolutionAction =
  | { kind: 'keep_current' }
  | { kind: 'keep_local' }
  | { kind: 'replace'; value: string }
  | { kind: 'clear' };

export interface SourceDocumentCredentialResolution {
  path: string;
  action: SourceDocumentCredentialResolutionAction;
}

export interface RebaseSourceDocumentRequest {
  pin_id: string;
  document_id: SourceDocumentId;
  base_revision: number;
  current_revision: number;
  local_masked_text: string;
  mode: SourceDocumentRebaseMode;
  credential_resolutions: SourceDocumentCredentialResolution[];
}

export interface InstallSourceCandidateRequest {
  candidate_id: string;
  grant: CapabilityGrantPreset;
}

export type ListSourceDocumentsRequest = Record<string, never>;

export interface GetSourceDocumentRequest {
  document_id: SourceDocumentId;
}

export interface CreateSourceDocumentRequest {
  format: SourceDocumentFormat;
  title: string;
  text: string;
}

export interface SaveSourceDocumentRequest {
  document_id: SourceDocumentId;
  expected_revision: number;
  masked_text: string;
}

export interface RenameSourceDocumentRequest {
  document_id: SourceDocumentId;
  expected_revision: number;
  title: string;
}

export interface DeleteSourceDocumentRequest {
  document_id: SourceDocumentId;
  expected_revision: number;
}

export interface SourceDocumentCredentialTarget {
  document_id: SourceDocumentId;
  document_revision: number;
  slot_id: CredentialSlotId;
}

export interface RevealSourceDocumentCredentialRequest {
  target: SourceDocumentCredentialTarget;
}

export interface ClearSourceDocumentCredentialRequest {
  target: SourceDocumentCredentialTarget;
}

export interface ReplaceSourceDocumentCredentialRequest {
  target: SourceDocumentCredentialTarget;
  value: string;
}

export interface SourceDocumentCredentialReveal {
  target: SourceDocumentCredentialTarget;
  value: string;
}

export interface DocumentValidationIssue {
  code: string;
  message: string;
  path: string | null;
  byte_offset: number | null;
  byte_length: number | null;
}

export type DocumentMutationOutcome =
  | { status: 'saved'; document: MaskedSourceDocument | null }
  | {
      status: 'conflict';
      expected_revision: number;
      actual_revision: number;
      current: MaskedSourceDocument;
    }
  | { status: 'not_found' }
  | { status: 'locked' }
  | { status: 'key_unavailable' }
  | { status: 'key_lost' }
  | { status: 'corrupt' }
  | { status: 'invalid'; issues: DocumentValidationIssue[] };

export function listSourceDocuments(): Promise<SourceDocumentSummary[]> {
  return invoke<SourceDocumentSummary[]>('list_source_documents', { request: {} });
}

export function createSourceDocument(
  request: CreateSourceDocumentRequest,
): Promise<DocumentMutationOutcome> {
  return invoke<DocumentMutationOutcome>('create_source_document', { request });
}

export function getSourceDocument(
  request: GetSourceDocumentRequest,
): Promise<MaskedSourceDocument | null> {
  return invoke<MaskedSourceDocument | null>('get_source_document', { request });
}

export function saveSourceDocument(
  request: SaveSourceDocumentRequest,
): Promise<DocumentMutationOutcome> {
  return invoke<DocumentMutationOutcome>('save_source_document', { request });
}

export function renameSourceDocument(
  request: RenameSourceDocumentRequest,
): Promise<DocumentMutationOutcome> {
  return invoke<DocumentMutationOutcome>('rename_source_document', { request });
}

export function deleteSourceDocument(
  request: DeleteSourceDocumentRequest,
): Promise<DocumentMutationOutcome> {
  return invoke<DocumentMutationOutcome>('delete_source_document', { request });
}

export function revealSourceDocumentCredential(
  request: RevealSourceDocumentCredentialRequest,
): Promise<SourceDocumentCredentialReveal> {
  return invoke<SourceDocumentCredentialReveal>('reveal_source_document_credential', { request });
}

export function replaceSourceDocumentCredential(
  request: ReplaceSourceDocumentCredentialRequest,
): Promise<DocumentMutationOutcome> {
  return invoke<DocumentMutationOutcome>('replace_source_document_credential', { request });
}

export function clearSourceDocumentCredential(
  request: ClearSourceDocumentCredentialRequest,
): Promise<DocumentMutationOutcome> {
  return invoke<DocumentMutationOutcome>('clear_source_document_credential', { request });
}

export function pinSourceDocumentRevision(
  request: PinSourceDocumentRevisionRequest,
): Promise<SourceDocumentRevisionPin> {
  return invoke<SourceDocumentRevisionPin>('pin_source_document_revision', { request });
}

export function releaseSourceDocumentRevisionPin(
  request: ReleaseSourceDocumentRevisionPinRequest,
): Promise<SourceDocumentRevisionPinReleased> {
  return invoke<SourceDocumentRevisionPinReleased>('release_source_document_revision_pin', {
    request,
  });
}

export function rebaseSourceDocument(
  request: RebaseSourceDocumentRequest,
): Promise<DocumentMutationOutcome> {
  return invoke<DocumentMutationOutcome>('rebase_source_document', { request });
}

export function installSourceCandidate(
  request: InstallSourceCandidateRequest,
): Promise<InstalledSource> {
  return invoke<InstalledSource>('install', { request });
}

export function prepareInstallFromDocument(request: DocumentRef): Promise<InstallCandidate> {
  return invoke<InstallCandidate>('prepare_install_from_document', { request });
}
