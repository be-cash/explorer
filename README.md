# (Blockchain) Explorer

A no-frills explorer focused on speed and providing in-depth information


## Get Started

### 1. Setup local dependency

For the time being there's a local dependency that must be manually placed before proceeding with the regular setup.

To satisfy this dependency you need to clone [LogosFoundation/bitcoinsuite](https://github.com/LogosFoundation/bitcoinsuite/) on the same level as this repository. If you check [explorer-server/Cargo.toml](explorer-server/Cargo.toml) you'll see that Cargo expects this folder to be two levels up.

```toml
[dependencies]
bitcoinsuite-chronik-client = { path = "../../bitcoinsuite/bitcoinsuite-chronik-client" }
bitcoinsuite-error =  { path = "../../bitcoinsuite/bitcoinsuite-error" }
bitcoinsuite-core =  { path = "../../bitcoinsuite/bitcoinsuite-core" }
```

Your folder structure should look similar to this:

```
bitcoinsuite
├── bitcoinsuite-*
├── chronik-client
├── Cargo.lock
├── Cargo.toml
├── LICENSE
└── Makefile.toml
explorer
├── Cargo.lock
├── Cargo.toml
└── explorer-server
```

### 2. Configure & Run

```
cd explorer-exe
cp config.dist.toml config.toml
```

The ([config.dist.toml](explorer-server/config.dist.toml)) is just an example on which to base your config from:

```toml
host = "0.0.0.0:3035"
chronik_api_url = "https://chronik.be.cash/xec"
```

You're all done! Now you can run the project:
```
cargo run
```

Go to http://localhost:3035 and you should see the homepage

## Supported Chains

- [x] eCash XEC
- [ ] Lotus XPI (coming soon)
