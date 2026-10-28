import {spawn} from 'node:child_process';
import {isAbsolute} from 'node:path';
import {tmpdir} from 'node:os';
import {LocalAIError} from '../dist/errors.js';

/** The same explicit environment policy applies to brokers and bridges. */
export function processOptions({cwd, env: overrides} = {}) {
  if (cwd !== undefined && (typeof cwd !== 'string' || !isAbsolute(cwd) || cwd.includes('\0'))) throw new LocalAIError('INVALID_WORKSPACE');
  if (overrides !== undefined && (overrides === null || typeof overrides !== 'object' || Array.isArray(overrides))) throw new LocalAIError('INVALID_ENVIRONMENT');
  const env = {};
  for (const key of ['PATH','Path','SystemRoot','SYSTEMROOT','WINDIR','HOME','USERPROFILE','TMP','TEMP','TMPDIR','LANG','XDG_CONFIG_HOME','XDG_DATA_HOME','XDG_STATE_HOME','XDG_RUNTIME_DIR','APPDATA','LOCALAPPDATA','LOCAL_AI_KIT_STATE_DIR','ORIGINLM_STATE_DIR','DBUS_SESSION_BUS_ADDRESS']) {
    if (process.env[key] !== undefined) env[key] = process.env[key];
  }
  for (const [key, value] of Object.entries(overrides ?? {})) {
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(key) || typeof value !== 'string' || value.includes('\0') || ['NODE_OPTIONS','LD_PRELOAD','DYLD_INSERT_LIBRARIES','PYTHONSTARTUP','PYTHONPATH','PYTHONHOME'].includes(key.toUpperCase())) throw new LocalAIError('INVALID_ENVIRONMENT');
    env[key] = value;
  }
  return {cwd: cwd ?? tmpdir(), env};
}

/** Normalize synchronous OS startup failures without exposing native details. */
export function spawnHelper(executable, args, options) {
  try { return spawn(executable, args, options); }
  catch { throw new LocalAIError('RUNTIME_MISSING'); }
}
