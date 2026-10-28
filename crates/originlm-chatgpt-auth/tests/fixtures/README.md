# Synthetic signing fixture

`test-rsa.pem` is a fixed test-only RSA key inherited from the source test suite. It signs synthetic JWTs for deterministic verifier tests and must never be used for an account, server or release. It is deliberately tracked despite the default private-key ignore rule.
