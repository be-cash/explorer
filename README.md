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
cd explorer-exe/
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

### 3. Build

1. `cd` into explorer/explorer-exe and run `cargo build --release` (will take a while). You might need to install some required libraries.
2. Compiled binary will be in `explorer/target/release/explorer-exe`. Copy it to `explorer/explorer-exe`
3. It is recommended to run `cargo clean` in both `bitcoinsuite` and `explorer` afterwards (will delete `explorer/target/release/explorer-exe` executable), as compilation artifacts can take up a lot of space.

Now you can run the project with `./explorer/explorer-exe/explorer-exe`

### 4. Production Deployment

One option is to run the app with `systemctl`

First, create your `explorer.service` file

`sudo nano /etc/systemd/system/explorer.service`

Sample contents (may be modified according to your deployment preferences):

```
#explorer.service
[Service]
ExecStart=/path/to/explorer/explorer-exe/explorer-exe /path/to/explorer/explorer-exe/
Restart=always
StandardOutput=syslog
StandardError=syslog
SyslogIdentifier=explorer
User=yourUser
Group=yourGroup
[Install]
WantedBy=multi-user.target
```

Now you can start the explorer with

`sudo systemctl start explorer`

Check logs with

`sudo journalctl -u explorer`

Check logs in real time

`sudo journalctl -u explorer -f`

Check logs for the last day

`sudo journalctl -u explorer --since today`

## Supported Chains

- [x] eCash XEC
- [ ] Lotus XPI (coming soon)
