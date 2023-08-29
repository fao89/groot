![Continuous Integration](https://github.com/fao89/groot/workflows/Continuous%20Integration/badge.svg)
![license](https://img.shields.io/crates/l/groot)
![Latest version](https://img.shields.io/crates/v/groot.svg)
![Downloads](https://img.shields.io/crates/d/groot)
# I am Groot!
![groot](https://www.redringtones.com/wp-content/uploads/2019/04/i-am-groot-ringtone.jpg)

## Required variables
Please have a `.env` file with the following variables:
- `SERVER.HOST`: The host address e.g. `127.0.0.1`
- `SERVER.PORT`: The host port e.g. `3030`
- `DATABASE_URL`: The postgres DB URL e.g. `postgres://groot:groot@localhost:5432/groot`
- `REDIS_URL`: The redis URL e.g. `redis://redis:6379`

## Downloading
```console
$ curl -L https://github.com/fao89/groot/releases/download/0.4.0/groot-linux-amd64 -o groot
$ chmod +x groot
```
## Sync roles or collections

Mirror:
```console
$ curl -X POST http://127.0.0.1:3030/sync/<roles | collections>
```

From requirements.yml
```console
$ curl -X POST -F 'requirements=@requirements.yml' http://127.0.0.1:3030/sync/
```

## Upload collections

```console
ansible-galaxy collection publish -c -s http://127.0.0.1:3030/ <COLLECTION_TARBALL_PATH>
```

## Serving content
```console
$ curl -L https://github.com/fao89/groot/releases/download/0.4.0/groot-linux-amd64 -o groot
$ chmod +x groot
$ ./groot
```
Install role/collection from groot:
```console
$ ansible-galaxy <role | collection> install <namespace>.<name> -c -s http://127.0.0.1:3030/
```
