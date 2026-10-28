"""Sanitized errors: provider response bodies, tokens, and local paths never enter SDK errors."""
import re


class LocalAIError(Exception):
    def __init__(self, code: str, retryable: bool = False, retry_after_ms: int | None = None):
        self.code = code if re.fullmatch(r'[A-Z][A-Z0-9_]{0,63}', code) else 'PROTOCOL_INVALID'
        self.retryable = retryable is True
        self.retry_after_ms = retry_after_ms
        super().__init__(f'Local AI request failed ({self.code})')


def remote_error(data) -> LocalAIError:
    if not isinstance(data, dict):
        return LocalAIError('REMOTE_ERROR')
    code = data.get('code') if isinstance(data.get('code'), str) else 'REMOTE_ERROR'
    retryable = data.get('retryable') is True
    retry_after_ms = data.get('retryAfterMs')
    if retry_after_ms is not None and (
        not retryable or type(retry_after_ms) is not int or not 0 <= retry_after_ms <= 86_400_000
    ):
        return LocalAIError('PROTOCOL_INVALID')
    return LocalAIError(code, retryable, retry_after_ms)
