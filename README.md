# Overview

This project is part of my [CI/CD exploration](https://github.com/cedrc-pr/infra); it was created to answer the following question: if GitHub Actions build and push Docker images, how does the server know new images are available ? We have a number of options available to us:

- GitHub Actions runs commands through an SSH connection. This is a bad idea because you would need to grant the ‘docker’ group to that account, which is dangerous.
- GitHub Actions does nothing; the server periodically checks for updates.
- GitHub Actions calls an API on the server using a token in the ‘Bearer’ header to trigger the execution of a script.

I chose the most elegant option, the third one. This project is a Rust API that uses a configuration file to define which services it can update with their script.

# Development setup

Run :

```shell
cp .env.example .env
cp deployer.example.conf deployer.conf
```

You can create, a test script `./foo.sh` and write in `./deployer.conf`:

```conf
my-foo-service;./foo.sh;abc
```

Run the API:

```shell
cargo run
```

And you can try it with [httpie](https://httpie.io/cli) for example:

```shell
http :3000/ "Authorization:Bearer abc" service=my-foo-service
```

You can reload the `deployer.conf` without restarting the API, just send it a hang up signal:

```shell
kill -s HUP PID
```

With tracing in debug level, you can see the process id at the beginning.

# Deployment

The API cannot be containerised because it would not be able to run scripts on the host (it could, but it would not be secure). So it must be converted into a daemon and here is how to do it.

Create a `/etc/systemd/system/deployer.service` file and write in it:

```service
[Unit]
Description=Script execution API

Requires=docker.service
After=docker.service

[Service]
Type=simple

User=deployer
Group=deployer
SupplementaryGroups=docker

ExecStart=/usr/local/bin/deployer

Restart=on-failure
RestartSec=5

EnvironmentFile=-/etc/deployer/.env

[Install]
WantedBy=multi-user.target
```

In `/etc/deployer/.env`:

```env
BIND_ADDR="127.0.0.1:3000"
CONF_PATH="/etc/deployer/deployer.conf"
```

Place the binary:

```shell
cargo build --release
cp target/release/deployer /usr/local/bin/deployer
```

To run this service:

```shell
sudo systemctl daemon-reload
sudo systemctl enable --now deployer.service
# and see its logs
journalctl -u deployer.service
```

# An important detail

The API is deployed on my home server, which runs a traefik container as a reverse-proxy; and I want to use it as my only entry.

But how a container could talk to a daemon on the host without sharing the host network (`network_mode: host`) ?

By:

- creating a docker subnet
- connecting the traefik container to it
- binding the daemon on it

Because they are in the same subnet, the traefik container will be able to talk the the API.

So the API is not directly exposed, requests will have to pass by traefik (with a configured router in it) to talk to the API.
