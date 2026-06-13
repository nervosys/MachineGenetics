# MAGE Governance

This document describes the governance structure for the MAGE programming
language project.

## Roles

| Role                 | Responsibility                        | Current Members      |
| -------------------- | ------------------------------------- | -------------------- |
| **Core Team**        | Language design, compiler development | nervosys maintainers |
| **SKB Curators**     | Safety Knowledge Base rule curation   | TBD                  |
| **Forge Moderators** | Package registry quality control      | TBD                  |
| **RFC Authors**      | Design proposals (MAGE RFC process)  | Open to all          |
| **ACI Trainers**     | AI Coding Intelligence model training | TBD                  |

## Decision Making

### Day-to-Day Decisions

- Bug fixes, documentation improvements, and small features are decided by
  the PR reviewer and maintainer.
- Any maintainer can merge PRs that pass CI and have at least one approval.

### Language Design Decisions

Significant changes to MAGE syntax, semantics, or the ecosystem require the
RFC process:

1. **Proposal** — Author writes an RFC using `community/rfc-template.md`
2. **Discussion** — Community discusses in the PR for at least 14 days
3. **Final Comment Period (FCP)** — Core team signals intent to accept/reject
   with a 7-day FCP
4. **Decision** — Core team merges or closes the RFC PR
5. **Implementation** — Accepted RFCs are tracked via GitHub Issues

### SKB Rule Changes

Safety Knowledge Base rule changes follow a stricter process:

1. Propose the rule with formal semantics
2. Demonstrate the rule on at least 3 real-world code examples
3. Show that the rule does not break existing valid code
4. Two SKB curators must approve
5. The rule is added to `skb/` with full documentation

## Editions

MAGE uses an edition system for backward-compatible evolution:

| Edition | Year  | Key Features                                                   |
| ------- | :---: | -------------------------------------------------------------- |
| 2025    | 2025  | Core language, basic SKB, LL(1) parser                         |
| 2026    | 2026  | Full MLIR pipeline, effect system, agent primitives            |
| 2027    | 2027  | Full ACI, Cost Oracle, hot-reload, swarm orchestration         |
| 2028+   | 2028  | Self-hosting compiler, advanced synthesis, formal verification |

Edition migration is automated via `mg migrate --edition <year>`.

## Code of Conduct

All participants in the MAGE project must follow the
[Code of Conduct](../CODE_OF_CONDUCT.md). The core team is responsible for
enforcing the Code of Conduct and may take action against participants who
violate it.

## Communication Channels

| Channel            | Purpose                           |
| ------------------ | --------------------------------- |
| GitHub Issues      | Bug reports, feature requests     |
| GitHub Discussions | Questions, design discussions     |
| GitHub PRs         | Code review, RFC discussion       |
| Discord (planned)  | Real-time chat, community support |

## Becoming a Contributor

1. Start with issues labeled `good-first-issue`
2. Submit PRs for documentation, training data, or bug fixes
3. Consistent contributors may be invited to join the core team
4. All contributions are recognized in release notes
