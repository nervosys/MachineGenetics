# Community

This directory contains community infrastructure for the MechGen programming
language project: contribution guidelines, governance, issue/PR templates, and
the RFC process.

## Directory Structure

```
community/
├── README.md                   # This file
├── CONTRIBUTING.md             # How to contribute
├── GOVERNANCE.md               # Project governance and roles
├── rfc-template.md             # Template for language RFCs
├── rfcs/                       # Accepted RFCs (proposals)
│   └── .gitkeep
├── pull-request-template.md    # PR template
└── issue-templates/
    ├── config.yml              # Issue template chooser config
    ├── bug_report.yaml         # Bug report template
    ├── feature_request.yaml    # Feature request template
    └── transpiler_issue.yaml   # Transpiler-specific issue template
```

## Quick Links

| Resource                                | Description                             |
| --------------------------------------- | --------------------------------------- |
| [CONTRIBUTING.md](CONTRIBUTING.md)      | How to contribute to MechGen              |
| [GOVERNANCE.md](GOVERNANCE.md)          | Roles, decision-making, editions        |
| [rfc-template.md](rfc-template.md)      | Template for proposing language changes |
| [Issue Templates](issue-templates/)     | GitHub issue form templates             |
| [PR Template](pull-request-template.md) | Pull request checklist                  |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full guide. The short version:

1. Check existing issues and discussions
2. Fork the repo, create a branch
3. Make your changes with tests
4. Open a PR using the template
5. Address review feedback

## RFC Process

For significant language changes:

1. Copy `rfc-template.md` to `rfcs/0000-your-proposal.md`
2. Fill in all sections
3. Open a PR titled `RFC: Your Proposal Title`
4. Discuss for at least 14 days
5. Core team decides after Final Comment Period

## Governance

See [GOVERNANCE.md](GOVERNANCE.md) for the full governance document, including
roles, decision-making processes, and edition planning.
