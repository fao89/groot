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

## Downloading
```console
$ curl -L https://github.com/fao89/groot/releases/download/0.3.0/groot-linux-amd64 -o groot
$ chmod +x groot
```
## Sync roles or collections

Mirror:
- Client-side:
```console
$ ./groot sync --content <roles | collections>
```

- Server-side:
```console
$ curl -X POST http://127.0.0.1:3030/sync/<roles | collections>
```

From requirements.yml
- Client-side:
```console
$ ./groot sync --requirement requirements.yml
```

- Server-side:
```console
$ curl -X POST -F 'requirements=@requirements.yml' http://127.0.0.1:3030/sync/
```

## Serving content
```console
$ curl -L https://github.com/fao89/groot/releases/download/0.3.0/groot-linux-amd64 -o groot
$ chmod +x groot
$ ./groot --serve
```
Install role/collection from groot:
```console
$ ansible-galaxy <role | collection> install <namespace>.<name> -c -s http://127.0.0.1:3030/
```
