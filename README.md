# rindex - A Indexer ~~to replace vindex~~

Not so fast but enough, intergrated with colorful, file based log.

## Usage

```bash
$ ./rindex --help
Usage: rindex -d <directory> [-a <address>] [-p <port>] [-t <threads>] [-f <logdir>] [-v]

Fast Indexer compatible with nginx's autoindex module.

Options:
  -d, --directory   base dir of the indexer
  -a, --address     ip address for listening
  -p, --port        port for listening
  -t, --threads     number of threads of web server
  -f, --logdir      directory of log files, empty for disable
  -v, --verbose     will show logs in stdout
  --help            display usage information
```
