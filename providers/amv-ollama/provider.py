"""One explicit local record-evidence call; owned by Aniflow's process runtime.

No media, model installation, fallback, tools or cloud endpoint is supported.
API model/version/digest observations are service reports, not weight attestation.
This standard-library adapter is not a network sandbox. Execute only through
the registered parent runtime, which owns deadlines/cancellation/output limits.
"""
import http.client
import ipaddress
import json
import os
from pathlib import Path
import re
import sys
import time


class Refused(Exception):
    pass


def require(condition):
    if not condition:
        raise Refused()


def unique_object(pairs):
    value = {}
    for key, member in pairs:
        require(key not in value)
        value[key] = member
    return value


def decode(data):
    return json.loads(data, object_pairs_hook=unique_object,
                      parse_constant=lambda _: (_ for _ in ()).throw(Refused()))


def endpoint(value):
    if value.startswith("["):
        host, separator, port = value[1:].partition("]:")
    else:
        host, separator, port = value.rpartition(":")
    require(separator and ipaddress.ip_address(host).is_loopback)
    require(port.isascii() and port.isdigit() and 0 < int(port) < 65536)
    return host, int(port)


def call(address, deadline, method, path, value=None):
    require((method, path) in {("GET", "/api/version"), ("GET", "/api/tags"),
                                ("POST", "/api/chat")})
    remaining = deadline - time.monotonic()
    require(remaining > 0)
    body = None if value is None else json.dumps(value, ensure_ascii=False,
                                                separators=(",", ":")).encode()
    require(body is None or len(body) <= 131072)
    connection = http.client.HTTPConnection(*address, timeout=remaining)
    try:
        connection.request(method, path, body=body,
                           headers={"Content-Type": "application/json",
                                    "Accept-Encoding": "identity",
                                    "Connection": "close"})
        response = connection.getresponse()
        headers = response.getheaders()
        require(response.status == 200 and sum(len(k) + len(v) for k, v in headers) <= 16384)
        lengths = response.headers.get_all("Content-Length", [])
        encodings = response.headers.get_all("Transfer-Encoding", [])
        require(len(lengths) <= 1 and len(encodings) <= 1 and not (lengths and encodings))
        require(response.getheader("Content-Encoding", "identity") == "identity")
        require((lengths and lengths[0].isascii() and lengths[0].isdigit()
                 and int(lengths[0]) <= 262144)
                or (encodings and encodings[0].lower() == "chunked"))
        data = response.read(262145)
        require(len(data) <= 262144 and time.monotonic() <= deadline)
        if lengths:
            require(len(data) == int(lengths[0]))
        return decode(data)
    finally:
        connection.close()


def observe(request, address, deadline):
    version = call(address, deadline, "GET", "/api/version")
    require(isinstance(version, dict) and version.get("version") == request["runtime_version"])
    tags = call(address, deadline, "GET", "/api/tags")
    require(isinstance(tags, dict) and isinstance(tags.get("models"), list)
            and len(tags["models"]) <= 256)
    found = [m for m in tags["models"] if isinstance(m, dict)
             and m.get("name") == request["model_name"]]
    require(len(found) == 1 and found[0].get("digest") == request["model_sha256"])


def usage(value, key, maximum):
    number = value.get(key)
    require(number is None or (type(number) is int and 0 <= number <= maximum))
    return number


def invoke(request):
    require(isinstance(request, dict) and set(request) == {
        "schema", "packet", "identity", "endpoint", "runtime_version", "model_name",
        "model_sha256", "service_egress_disabled_declared", "context_tokens", "output_tokens",
        "timeout_milliseconds", "template", "candidate_schema"})
    require(request["schema"] == "aniflow.amv-ollama-invocation/v1"
            and request["service_egress_disabled_declared"] is True)
    require(type(request["context_tokens"]) is int and 512 <= request["context_tokens"] <= 32768)
    require(type(request["output_tokens"]) is int and 0 < request["output_tokens"] <= 2048
            and request["output_tokens"] < request["context_tokens"])
    require(type(request["timeout_milliseconds"]) is int and 0 < request["timeout_milliseconds"] <= 300000)
    require(isinstance(request["model_name"], str)
            and re.fullmatch(r"[A-Za-z0-9._:/-]{1,48}", request["model_name"]))
    require(isinstance(request["model_sha256"], str)
            and re.fullmatch(r"[a-f0-9]{64}", request["model_sha256"]))
    packet = request["packet"]
    require(isinstance(packet, dict) and packet.get("schema") == "aniflow.amv-evidence-packet/v1"
            and packet.get("media_samples") == 0 and packet.get("full_movie_reviewed") is False)
    require(isinstance(request["template"], str) and len(request["template"].encode()) <= 2048)
    address = endpoint(request["endpoint"])
    deadline = time.monotonic() + request["timeout_milliseconds"] / 1000
    keys = ["observed_fact_ids", "suggestions", "questions"]
    schema = {"type": "object", "additionalProperties": False, "required": keys,
              "properties": {k: request["candidate_schema"]["properties"][k] for k in keys}}
    observe(request, address, deadline)
    reply = call(address, deadline, "POST", "/api/chat", {
        "model": request["model_name"], "stream": False, "think": False,
        "keep_alive": 0, "format": schema,
        "options": {"temperature": 0, "num_ctx": request["context_tokens"],
                    "num_predict": request["output_tokens"]},
        "messages": [{"role": "system", "content": request["template"]},
                     {"role": "user", "content": json.dumps(packet, ensure_ascii=False,
                                                               separators=(",", ":"))}]})
    require(isinstance(reply, dict) and reply.get("done") is True
            and reply.get("done_reason") == "stop" and reply.get("model") == request["model_name"])
    message = reply.get("message")
    require(isinstance(message, dict) and message.get("role") == "assistant"
            and not message.get("tool_calls") and not message.get("thinking"))
    text = message.get("content")
    require(isinstance(text, str) and len(text.encode()) <= 65536)
    notes = decode(text)
    require(isinstance(notes, dict) and set(notes) == set(keys)
            and all(isinstance(notes[k], list) and len(notes[k]) <= 16 for k in keys))
    observe(request, address, deadline)
    candidate = {"schema": "aniflow.amv-advisory-candidate/v1",
                 "packet_sha256": packet["packet_sha256"],
                 "declared_identity": request["identity"], **notes}
    return {"schema": "aniflow.amv-ollama-result/v1", "candidate": candidate,
            "runtime_version_reported": request["runtime_version"],
            "model_name_reported": request["model_name"],
            "model_sha256_reported": request["model_sha256"], "model_calls": 1,
            "input_tokens": usage(reply, "prompt_eval_count", request["context_tokens"]),
            "output_tokens": usage(reply, "eval_count", request["output_tokens"])}


def main():
    require(len(sys.argv) == 5 and sys.argv[1] == "--input" and sys.argv[3] == "--output")
    source, output = Path(sys.argv[2]), Path(sys.argv[4])
    require(source.is_absolute() and output.is_absolute() and source.resolve() == source
            and output.parent.resolve() == output.parent and not output.exists())
    with source.open("rb") as handle:
        data = handle.read(131073)
    require(len(data) <= 131072)
    encoded = json.dumps(invoke(decode(data)), ensure_ascii=False,
                         separators=(",", ":")).encode()
    require(0 < len(encoded) <= 131072)
    descriptor = os.open(output, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(descriptor, "wb") as handle:
        handle.write(encoded)
        handle.flush()
        os.fsync(handle.fileno())


if __name__ == "__main__":
    try:
        main()
    except Exception:
        # Fixed diagnostic only. HTTP bodies, prompts, notes and private paths
        # never enter the parent runtime's captured diagnostics.
        sys.stderr.write("local_amv_provider_refused\n")
        sys.exit(2)
