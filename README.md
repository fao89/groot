![Continuous Integration](https://github.com/fao89/groot/workflows/Continuous%20Integration/badge.svg)
# I am Groot!
![groot](https://www.redringtones.com/wp-content/uploads/2019/04/i-am-groot-ringtone.jpg)

## Sync roles or collections

Mirror:
```
$ curl -L https://github.com/fao89/groot/releases/download/0.2.0/groot-linux-amd64 -o groot
$ chmod +x groot
$ ./groot sync --content <roles | collections>
```

From requirements.yml
```
$ curl -L https://github.com/fao89/groot/releases/download/0.2.0/groot-linux-amd64 -o groot
$ chmod +x groot
$ ./groot sync --requirement requirements.yml
```

## Serve content
```
$ curl -L https://github.com/fao89/groot/releases/download/0.2.0/groot-linux-amd64 -o groot
$ chmod +x groot
$ RUST_LOG=groot::api ./groot --serve
```
Install role/collection from groot:
```
$ ansible-galaxy <role/collection> install <namespace>.<name> -c -s http://127.0.0.1:3030/ --no-deps
```
