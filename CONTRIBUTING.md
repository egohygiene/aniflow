# Contributing

Keep changes small, reproducible, and grounded in a real media workflow.

Before opening a pull request:

```bash
task validate
```

Pull requests should:

- explain the user-visible pipeline behavior being changed;
- preserve source safety and path handling;
- include unit coverage for configuration or planning changes;
- extend the synthetic smoke test when execution behavior changes;
- follow the canonical [architecture graph](docs/architecture/README.md);
- update the pipeline schema, architecture documents, and decision log when
  contracts, ownership, or invariants change;
- use Conventional Commits.

Avoid adding a general abstraction until at least two concrete processors or
stages require it.
