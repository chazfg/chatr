## Getting Started

### Prequsites

1. Install rust
2. ```
   git clone https://github.com/chazfg/chatr.git
   cd chatr
   ```

### Running the server

```sh
cargo run --bin server -- [HOST] [BANNED_USERNAMES]
```

banned_usernames can be a path to a file containing comma delimited usernames or just a comma delimited list inline

### Running the TUI client

```sh
cd client_chatr
cargo run
```
