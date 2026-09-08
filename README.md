# ChainLens

An Ethereum blockchain indexer with reorg-safe, crash-resumable ingestion.

## Repository structure

```
ChainLens/
├── private/          Core implementation (private repository)
│   ├── src/          Rust source code
│   ├── migrations/   Database schema
│   ├── tests/        Integration tests
│   ├── benches/      Benchmarks and fixtures
│   └── ...
│
└── public/           Published results and website (public repository)
    ├── README.md     Public documentation
    ├── website/      Results dashboard
    ├── data/         Published benchmark results
    └── docs/         Methodology documentation
```

See [public/README.md](public/README.md) for the public-facing project description.
