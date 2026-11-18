The engine's Claude Code setup, as withhuman-core keeps it in `deploy/client` (`configure.py` and
`hooks/pre-tool-use.py`). The developer's app writes these two files to its settings folder and
runs `configure.py` to install the hooks, for one folder (`--scope project`) or every folder of
the user (`--scope user`). Refresh them with `scripts/sync-client.sh <withhuman-core>`; do not edit
them here.
