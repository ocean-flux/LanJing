//! SessionError 的稳定 code → 本地化文案。
//!
//! 工具栏与编辑面各自捕获 session 抛出的错误，映射规则只应有一份。

import type { useMessages } from '@/shared/i18n/messages';
import { SessionError } from './model/session';

type Messages = ReturnType<typeof useMessages>;

export function sessionErrorText(m: Messages, error: unknown): string {
  if (!(error instanceof SessionError)) {
    return error instanceof Error ? error.message : String(error);
  }
  switch (error.code) {
    case 'document_not_found': {
      return m.rules_error_document_not_found();
    }
    case 'document_semantic_missing': {
      return m.rules_error_semantic_missing();
    }
    case 'document_semantic_unsaved': {
      return m.rules_unsaved_changes();
    }
    case 'document_validation_required': {
      return m.rules_validation_required();
    }
    case 'document_not_open': {
      return m.rules_error_document_not_open();
    }
  }
}
