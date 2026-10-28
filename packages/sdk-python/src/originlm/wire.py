import json
import re
from typing import Any
from .errors import LocalAIError


def encode_request(identifier: int, method: str, params: Any, max_bytes: int) -> bytes:
    if not isinstance(method, str) or len(method) > 128 or not re.fullmatch(r'[A-Za-z][A-Za-z0-9]*(?:\.[A-Za-z][A-Za-z0-9]*)+', method):
        raise LocalAIError('INVALID_METHOD')
    try:
        raw = (json.dumps({'jsonrpc':'2.0','id':identifier,'method':method,'params':params}, ensure_ascii=False, allow_nan=False, separators=(',', ':')) + '\n').encode('utf-8')
    except (TypeError, ValueError, UnicodeError):
        raise LocalAIError('INVALID_REQUEST') from None
    if len(raw) > max_bytes:
        raise LocalAIError('FRAME_TOO_LARGE')
    return raw


def decode_message(raw: bytes) -> dict:
    def reject_constant(_value):
        raise ValueError('invalid JSON constant')
    def unique_keys(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError('duplicate key')
            result[key] = value
        return result
    try:
        value = json.loads(raw.decode('utf-8'), parse_constant=reject_constant, object_pairs_hook=unique_keys)
    except (ValueError, UnicodeError):
        raise LocalAIError('PROTOCOL_INVALID') from None
    if not isinstance(value, dict) or value.get('jsonrpc') != '2.0':
        raise LocalAIError('PROTOCOL_INVALID')
    return value
