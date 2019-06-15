# sprinkler-k8s
Kubernetes DoS prevention

## Required configurations

```
/etc/sprinkler.conf.d/config.toml
/etc/sprinkler.conf.d/master.crt
/etc/sprinkler.conf.d/master.p12
/root/.sprinkler.key
```

## Build

```
cargo build --features sprinkler/master
```