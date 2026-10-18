"""Serve the working directory on an OS-assigned port and announce it.

usage: postcheck_ready_server.py READY_FILE

The listener is bound (port 0) before the port is written, and is kept open
for the life of the process, so the announced port cannot be taken by another
process in between.
"""

import os
import sys
from http.server import HTTPServer, SimpleHTTPRequestHandler


def main(ready_file):
    server = HTTPServer(("127.0.0.1", 0), SimpleHTTPRequestHandler)
    tmp = ready_file + ".tmp"
    with open(tmp, "w", encoding="utf-8") as handle:
        handle.write(str(server.server_address[1]))
    os.replace(tmp, ready_file)
    server.serve_forever()


if __name__ == "__main__":
    main(sys.argv[1])
