git pull origin master
cargo build --release --features sprinkler/master
scp target/release/sprinkler-agent root@k-prod-cpu-8.dsa.lan:/usr/local/bin
