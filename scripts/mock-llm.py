"""Tiny mock LLM upstream used by the e2e capture demo/test.

Answers every POST with a fixed completion that contains a recognizable code
block, so `re proxy` + `re watch` can demonstrate the content-based causal
join without real API keys. The wire shape follows the path and the request:

- `.../chat/completions`  OpenAI chat; `tools` in the request -> the code is
                          returned inside `tool_calls[].function.arguments`,
                          otherwise as plain `message.content`. With
                          `stream: true` the answer is SSE, and the usage chunk
                          is only sent when `stream_options.include_usage` is
                          set (as the real API does).
- `.../messages`          Anthropic messages; the code is returned in a
                          `tool_use` block (`input_json_delta` when streaming).
- `.../responses`         OpenAI Responses; the code is returned in a
                          `function_call` item (`response.function_call_arguments.delta`
                          when streaming).
- anything else           404, so mis-routed traffic is visible.
"""

import json
from http.server import BaseHTTPRequestHandler, HTTPServer

CODE = (
    "def refresh_token(user):\n"
    "    token = issue_token(user, scope=\"session\")\n"
    "    return rotate_every(token, hours=24)\n"
)
COMPLETION = "Here is the fix:\n```python\n" + CODE + "```\n"
FILE = "auth.py"
ARGS = json.dumps({"path": FILE, "content": CODE})


def chunks(s, n=7):
    return [s[i : i + n] for i in range(0, len(s), n)]


def openai_json(tools):
    if tools:
        message = {
            "role": "assistant",
            "content": None,
            "tool_calls": [
                {
                    "id": "call_mock",
                    "type": "function",
                    "function": {"name": "write_file", "arguments": ARGS},
                }
            ],
        }
    else:
        message = {"role": "assistant", "content": COMPLETION}
    return {
        "id": "chatcmpl-mock",
        "object": "chat.completion",
        "model": "gpt-4o-2024-08-06",
        "choices": [{"index": 0, "message": message, "finish_reason": "stop"}],
        "usage": {"prompt_tokens": 42, "completion_tokens": 18},
    }


def openai_sse(tools, include_usage):
    base = {"id": "chatcmpl-mock", "object": "chat.completion.chunk", "model": "gpt-4o-2024-08-06"}
    out = []

    def chunk(delta, finish=None):
        out.append(dict(base, choices=[{"index": 0, "delta": delta, "finish_reason": finish}]))

    if tools:
        chunk({"role": "assistant", "tool_calls": [{"index": 0, "id": "call_mock", "type": "function",
                                                     "function": {"name": "write_file", "arguments": ""}}]})
        for piece in chunks(ARGS):
            chunk({"tool_calls": [{"index": 0, "function": {"arguments": piece}}]})
        chunk({}, "tool_calls")
    else:
        chunk({"role": "assistant", "content": ""})
        for piece in chunks(COMPLETION):
            chunk({"content": piece})
        chunk({}, "stop")
    if include_usage:
        out.append(dict(base, choices=[], usage={"prompt_tokens": 42, "completion_tokens": 18}))
    return [("", c) for c in out] + [("", "[DONE]")]


def anthropic_json():
    return {
        "id": "msg_mock",
        "type": "message",
        "role": "assistant",
        "model": "claude-sonnet-4-20250514",
        "content": [
            {"type": "text", "text": "I'll write the helper."},
            {"type": "tool_use", "id": "toolu_mock", "name": "Write",
             "input": {"file_path": FILE, "content": CODE}},
        ],
        "stop_reason": "tool_use",
        "usage": {"input_tokens": 42, "output_tokens": 18},
    }


def anthropic_sse():
    tool_input = json.dumps({"file_path": FILE, "content": CODE})
    ev = [
        ("message_start", {"type": "message_start", "message": {
            "id": "msg_mock", "type": "message", "role": "assistant",
            "model": "claude-sonnet-4-20250514", "content": [],
            "usage": {"input_tokens": 42, "output_tokens": 1}}}),
        ("content_block_start", {"type": "content_block_start", "index": 0,
                                 "content_block": {"type": "text", "text": ""}}),
        ("content_block_delta", {"type": "content_block_delta", "index": 0,
                                 "delta": {"type": "text_delta", "text": "I'll write the helper."}}),
        ("content_block_stop", {"type": "content_block_stop", "index": 0}),
        ("content_block_start", {"type": "content_block_start", "index": 1,
                                 "content_block": {"type": "tool_use", "id": "toolu_mock",
                                                   "name": "Write", "input": {}}}),
    ]
    for piece in chunks(tool_input):
        ev.append(("content_block_delta", {"type": "content_block_delta", "index": 1,
                                           "delta": {"type": "input_json_delta", "partial_json": piece}}))
    ev += [
        ("content_block_stop", {"type": "content_block_stop", "index": 1}),
        ("message_delta", {"type": "message_delta", "delta": {"stop_reason": "tool_use"},
                           "usage": {"output_tokens": 18}}),
        ("message_stop", {"type": "message_stop"}),
    ]
    return ev


def responses_json():
    return {
        "id": "resp_mock",
        "object": "response",
        "model": "gpt-4.1-2025-04-14",
        "status": "completed",
        "output": [
            {"type": "message", "id": "msg_mock", "role": "assistant", "status": "completed",
             "content": [{"type": "output_text", "text": "Adding the helper.", "annotations": []}]},
            {"type": "function_call", "id": "fc_mock", "call_id": "call_mock",
             "name": "write_file", "arguments": ARGS, "status": "completed"},
        ],
        "usage": {"input_tokens": 42, "output_tokens": 18},
    }


def responses_sse():
    resp = {"id": "resp_mock", "object": "response", "model": "gpt-4.1-2025-04-14"}
    ev = [
        ("response.created", {"type": "response.created",
                              "response": dict(resp, status="in_progress", output=[])}),
        ("response.output_text.delta", {"type": "response.output_text.delta", "item_id": "msg_mock",
                                        "output_index": 0, "delta": "Adding the helper."}),
    ]
    for piece in chunks(ARGS):
        ev.append(("response.function_call_arguments.delta",
                   {"type": "response.function_call_arguments.delta", "item_id": "fc_mock",
                    "output_index": 1, "delta": piece}))
    ev.append(("response.completed", {"type": "response.completed",
                                      "response": dict(resp, status="completed",
                                                       usage={"input_tokens": 42, "output_tokens": 18})}))
    return ev


class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        length = int(self.headers.get("content-length", 0))
        raw = self.rfile.read(length)
        try:
            req = json.loads(raw) if raw else {}
        except ValueError:
            req = {}
        stream = bool(req.get("stream"))
        path = self.path.split("?", 1)[0].rstrip("/")

        if path.endswith("/chat/completions"):
            tools = bool(req.get("tools"))
            include_usage = bool((req.get("stream_options") or {}).get("include_usage"))
            if stream:
                return self.send_sse(openai_sse(tools, include_usage))
            return self.send_json(openai_json(tools))
        if path.endswith("/messages"):
            return self.send_sse(anthropic_sse()) if stream else self.send_json(anthropic_json())
        if path.endswith("/responses"):
            return self.send_sse(responses_sse()) if stream else self.send_json(responses_json())
        if path.endswith("/messages/count_tokens"):
            return self.send_json({"input_tokens": 42})
        self.send_response(404)
        self.end_headers()

    def send_json(self, obj):
        body = json.dumps(obj).encode()
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def send_sse(self, events):
        self.send_response(200)
        self.send_header("content-type", "text/event-stream")
        self.send_header("transfer-encoding", "chunked")
        self.end_headers()
        for name, data in events:
            payload = data if isinstance(data, str) else json.dumps(data)
            frame = (f"event: {name}\n" if name else "") + f"data: {payload}\n\n"
            self.write_chunk(frame.encode())
        self.wfile.write(b"0\r\n\r\n")

    def write_chunk(self, b):
        self.wfile.write(f"{len(b):x}\r\n".encode() + b + b"\r\n")

    def log_message(self, *args):
        pass


if __name__ == "__main__":
    import sys

    port = int(sys.argv[1]) if len(sys.argv) > 1 else 4399
    print(f"mock LLM upstream on http://127.0.0.1:{port}")
    HTTPServer(("127.0.0.1", port), Handler).serve_forever()
