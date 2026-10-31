# Universal product policy

OriginLM is a **universal product**. It is not a private integration layer for any one application, customer, or business.

## What is refused

Pull requests that add any of the following are refused:

- Application or product names, branding, copy, routes, IDs, feature flags, or workflows for a specific consumer.
- Customer- or tenant-specific rules, pricing, entitlements, quotas, billing logic, or business workflows.
- Hardcoded assumptions about one application's data model, UI, authentication, deployment, or storage.
- Tests or fixtures that encode a specific consumer's product behavior instead of a generic provider contract.
- A one-off capability that only makes sense for a single consumer.

The refusal reason is:

> **OriginLM is a universal product. Application-specific and business-specific logic belongs in the consuming application, not in this codebase.**

## What is accepted

- Generic provider adapters, protocol capabilities, configuration, and extension points that any application can use.
- Generic tests using synthetic providers, virtual homes, fake clients, or mock servers.
- Documentation that describes the generic contract rather than one consumer's product.
- A generic adapter or configuration that lets a consumer implement its own behavior outside OriginLM.

If a consumer needs behavior that OriginLM lacks, propose a **generic** capability. Do not add the consumer's name, workflow, or business rules to this repository.

## Enforcement

- Every pull request must attest to this policy in the pull-request template.
- Maintainers refuse a pull request that adds application- or business-specific logic, citing the refusal reason above.
- This is a semantic policy, not a name denylist. The repository does not hardcode consumer names or scan for a fixed list of applications.
- As real pull requests arrive, we may add generic structural checks. We will not add a consumer-name list.
