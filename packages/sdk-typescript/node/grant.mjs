import {constants, closeSync, fstatSync, lstatSync, openSync, readSync} from 'node:fs';

/** Bounded, owner-private descriptor read; imports have no I/O. */
export function readGrant(path, address) {
  try {
    const before = lstatSync(path);
    if (!before.isFile() || before.isSymbolicLink()) return null;
    const fd = openSync(path, constants.O_RDONLY | (constants.O_NOFOLLOW ?? 0) | (constants.O_NONBLOCK ?? 0));
    let descriptor;
    try {
      const info = fstatSync(fd);
      if (!info.isFile() || info.size > 16384 || info.dev !== before.dev || info.ino !== before.ino) return null;
      if (process.platform !== 'win32' && (info.mode & 0o077) !== 0) return null;
      const bytes = Buffer.alloc(16385);
      let length = 0;
      while (length < bytes.length) {
        const count = readSync(fd, bytes, length, bytes.length - length, null);
        if (!count) break;
        length += count;
      }
      if (length > 16384) return null;
      descriptor = JSON.parse(bytes.subarray(0, length).toString('utf8'));
    } finally { closeSync(fd); }
    if (descriptor?.address !== undefined && descriptor.address !== address) return null;
    if (typeof descriptor?.token !== 'string' || !/^[0-9a-f]{64}$/.test(descriptor.token)) return null;
    return descriptor;
  } catch {
    return null;
  }
}

