git pull origin master
cargo build --release --features sprinkler/master
for i in $(seq 1 9)
do
  scp target/release/sprinkler-agent root@k-prod-cpu-${i}.dsa.lan:/usr/local/bin
  scp systemd/sprinkler-agent.service root@k-prod-cpu-${i}.dsa.lan:/etc/systemd/system/sprinkler-agent.service
done

