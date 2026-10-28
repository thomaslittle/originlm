# ADR-008: Persist the update version floor before activation

Status: accepted, 2026-10-28.

A host may remain running after replacing a helper. Its compile-time version is
therefore not the installed version and cannot be the only downgrade boundary.
Under the activation lock, take the maximum of the compiled version, legacy
`state.json` installed version, and `version-floor.json` highest verified version.
Reject unreadable or malformed persisted version state rather than resetting it.

After verifying the signed replacement bytes, atomically persist and sync the new
floor before replacing the executable (including directory sync on Unix). Keep
installed-version metadata separate: an interrupted attempt may advance the floor
without installing the executable. Retrying that same version is allowed. Explicit
operator rollback restores the retained backup but does not lower the floor.

This conservative ordering prevents a process crash between replacement and the
informational metadata write from permitting a later downgrade. It is not protection
against the account owner deliberately deleting or modifying their update state.
Windows replacement and filesystem durability still require platform execution.
