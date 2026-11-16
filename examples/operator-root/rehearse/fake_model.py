"""A scripted stand-in for an OpenAI-compatible Chat Completions server.

It exists only to rehearse an operator root without a real model: it listens on
127.0.0.1, answers each POST to `.../chat/completions` with the next scripted
reply, and reports a synthetic billed cost so a run budget can settle each send.
It uses the Python standard library only. Start it with:

    python3 -I fake_model.py --replies replies.json --port 0 --record requests.jsonl

The first line on stdout is `listening http://127.0.0.1:<port>/v1`. Every
accepted connection and every request is appended to the record file as one
JSON line, so you can count what the host sent.

A reply in the script is one of:

    {"tool": "<tool name>", "arguments": {...}}   a native tool call
    {"text": "<words>"}                           a final answer

The string "{revision}" anywhere in `arguments` is replaced by the revision the
host's last tool answer names: its `revision` when the answer is `stale`, else
the first `stage.downstream[].revision`, else "initial". A `stale` answer makes
the fake repeat its previous call with that revision without moving on in the
script. After the last reply the fake keeps answering with the last `text`.
"""

import argparse
import http.server
import json
import socketserver
import sys
import threading

REVISION_TOKEN = "{revision}"


def last_tool_answer(body):
    """The host's last tool answer, decoded, or None."""
    for message in reversed(body.get("messages") or []):
        if message.get("role") != "tool":
            continue
        try:
            answer = json.loads(message.get("content") or "")
        except ValueError:
            return None
        return answer if isinstance(answer, dict) else None
    return None


def revision_from(answer):
    if not answer:
        return "initial"
    if answer.get("status") == "stale" and isinstance(answer.get("revision"), str):
        return answer["revision"]
    stage = answer.get("stage")
    if isinstance(stage, dict):
        for entry in stage.get("downstream") or []:
            if isinstance(entry, dict) and isinstance(entry.get("revision"), str):
                return entry["revision"]
    return "initial"


def fill(value, revision):
    if isinstance(value, str):
        return value.replace(REVISION_TOKEN, revision)
    if isinstance(value, list):
        return [fill(item, revision) for item in value]
    if isinstance(value, dict):
        return {key: fill(item, revision) for key, item in value.items()}
    return value


class Script:
    """The scripted model. One instance serves every request, one at a time."""

    def __init__(self, document):
        self.model = document.get("model", "example-model")
        self.cost = str(document.get("cost_per_reply", "0.0001"))
        self.replies = document["replies"]
        if not self.replies:
            raise ValueError("the script has no replies")
        self.position = 0
        self.previous_call = None
        self.calls = 0
        self.lock = threading.Lock()

    def next_message(self, body):
        with self.lock:
            answer = last_tool_answer(body)
            revision = revision_from(answer)
            if answer and answer.get("status") == "stale" and self.previous_call:
                step = self.previous_call
            else:
                index = min(self.position, len(self.replies) - 1)
                step = self.replies[index]
                self.position += 1
            if "tool" in step:
                self.previous_call = step
                self.calls += 1
                arguments = fill(step.get("arguments", {}), revision)
                return {
                    "role": "assistant",
                    "content": "",
                    "tool_calls": [
                        {
                            "id": "call-%d" % self.calls,
                            "type": "function",
                            "function": {
                                "name": step["tool"],
                                "arguments": json.dumps(arguments),
                            },
                        }
                    ],
                }
            return {"role": "assistant", "content": step.get("text", "")}


class Record:
    def __init__(self, path):
        self.path = path
        self.lock = threading.Lock()
        self.connections = 0
        self.requests = 0

    def write(self, entry):
        if self.path is None:
            return
        with self.lock, open(self.path, "a", encoding="utf-8") as handle:
            handle.write(json.dumps(entry, sort_keys=True) + "\n")

    def connection(self):
        with self.lock:
            self.connections += 1
            number = self.connections
        self.write({"event": "connection", "number": number})

    def request(self, path, body):
        with self.lock:
            self.requests += 1
            number = self.requests
        self.write({"event": "request", "number": number, "path": path, "body": body})


class Server(socketserver.TCPServer):
    allow_reuse_address = True

    def __init__(self, address, handler, script, record):
        self.script = script
        self.record = record
        super().__init__(address, handler)

    def verify_request(self, request, client_address):
        self.record.connection()
        return True


class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, format, *args):
        return

    def send_json(self, status, payload):
        data = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(data)))
        self.send_header("connection", "close")
        self.end_headers()
        self.wfile.write(data)
        self.close_connection = True

    def do_GET(self):
        self.server.record.request(self.path, None)
        self.send_json(404, {"error": {"message": "only chat completions are faked"}})

    def do_POST(self):
        length = int(self.headers.get("content-length") or 0)
        raw = self.rfile.read(length)
        try:
            body = json.loads(raw.decode("utf-8"))
        except ValueError:
            body = None
        self.server.record.request(self.path, body)
        if not self.path.endswith("/chat/completions") or not isinstance(body, dict):
            self.send_json(404, {"error": {"message": "only chat completions are faked"}})
            return
        script = self.server.script
        message = script.next_message(body)
        # Synthetic usage: about four bytes a token, never zero. The cost is the
        # script's fixed figure, sent as a JSON number the way a provider does.
        prompt_tokens = max(1, len(raw) // 4)
        completion_tokens = max(1, len(json.dumps(message)) // 4)
        reply = {
            "id": "fake-%d" % self.server.record.requests,
            "object": "chat.completion",
            "model": script.model,
            "choices": [{"index": 0, "message": message, "finish_reason": "stop"}],
            "usage": {
                "prompt_tokens": prompt_tokens,
                "completion_tokens": completion_tokens,
                "total_tokens": prompt_tokens + completion_tokens,
                "cost": json.loads(script.cost),
            },
        }
        self.send_json(200, reply)


def main(argv):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--replies", required=True, help="the scripted replies (JSON)")
    parser.add_argument("--port", type=int, default=0, help="0 picks a free port")
    parser.add_argument("--record", help="append each connection and request here")
    options = parser.parse_args(argv)
    with open(options.replies, encoding="utf-8") as handle:
        script = Script(json.load(handle))
    record = Record(options.record)
    server = Server(("127.0.0.1", options.port), Handler, script, record)
    port = server.server_address[1]
    sys.stdout.write("listening http://127.0.0.1:%d/v1\n" % port)
    sys.stdout.flush()
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
