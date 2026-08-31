# Contributing

Use the devcontainer and keep changes small and reviewable.

Before opening a pull request, run:

```shell
make check
make infrastructure-check
```

Do not edit generated domain code or upstream snapshots by hand. Change
`data/domain-policy.yaml` or run `make refresh`, then include the generated diff
in the pull request. A domain outside `gc.ca` or `canada.ca` requires evidence
that it is an email domain for the specified active organization; a website URL
alone is not sufficient.

Never use real personal email addresses in tests, issues, commit messages, or
logs. Test data should use clearly fictitious local parts or reserved example
domains.

Use Conventional Commit-style subjects where practical, for example
`feat(api): ...`, `fix(data): ...`, or `chore(deps): ...`.
