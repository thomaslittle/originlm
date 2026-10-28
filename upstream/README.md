# Upstream discovery knowledge

OriginLM discovers AI access that is already configured on a machine. Other open-source projects have accumulated real path, environment, and migration knowledge for those same clients. This directory records that knowledge as pinned, reviewed inputs.

```text
upstream knowledge sources
        │
        ├── Tokscale
        ├── OpenUsage        (planned)
        ├── AgentManager     (planned)
        └── official client repositories
                │
                ▼
       normalized discovery knowledge
                │
                ▼
        OriginLM adapters
```

Released builds do not clone, download, or execute an upstream project, and they do not follow an upstream `main` branch. A source is useful only after its discovery-relevant changes have been normalized, tested, and given provenance.

The manifest of sources is [sources.json](sources.json). Tokscale is the first pinned discovery source; see [tokscale/NOTICE.md](tokscale/NOTICE.md) and [the upstream guide](../docs/upstream/tokscale.md). T3 Code is pinned for provider detection and ChatGPT sign-in; see [t3code/NOTICE.md](t3code/NOTICE.md), [UPSTREAM_T3CODE.md](t3code/BASELINE.md), and [the T3 Code guide](../docs/upstream/t3code.md).

Authority, when sources disagree:

1. Official client implementation or protocol.
2. Direct OriginLM qualification.
3. Well-tested upstream open-source evidence.
4. Convention or inference.

A legacy fallback can still be kept when a higher-authority source simply does not mention it. Upstream evidence never becomes unquestioned truth.
