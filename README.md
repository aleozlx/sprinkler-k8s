# sprinkler-k8s
Kubernetes DoS prevention

## Required configurations

```
firewall-cmd --permanent --zone=public --add-rich-rule='rule family="ipv4" source address="?/32" port port="3777" protocol="tcp" accept'

/etc/sprinkler.conf.d/config.toml
/etc/sprinkler.conf.d/master.crt
/etc/sprinkler.conf.d/master.p12
/root/.sprinkler.key
```

## Build

```
cargo build --release --features sprinkler/master
```