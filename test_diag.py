#!/usr/bin/env python3
import subprocess, json, sys, time

proc = subprocess.Popen(
    [r"target/debug/zenlang.exe", "lsp"],
    stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE
)

def send(obj):
    msg = json.dumps(obj).encode("utf-8")
    header = f"Content-Length: {len(msg)}\r\n\r\n"
    proc.stdin.write(header.encode() + msg)
    proc.stdin.flush()

def recv():
    header = b""
    while b"\r\n\r\n" not in header:
        c = proc.stdout.read(1)
        if not c:
            return None
        header += c
    length = int(header.split(b":")[1].strip())
    return json.loads(proc.stdout.read(length))

# Initialize
send({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"processId":None,"capabilities":{},"rootUri":None}})
resp = recv()
print("=== Init OK ===")

# Send initialized
send({"jsonrpc":"2.0","method":"initialized","params":{}})

# Open doc
doc_uri = "file:///test.zen"
source = "fn main() -> int {\n\t//\"test\"\n\t42\n}\n\nfn testai() -> void {\n\n}\n\nfn an_error() -> int {\n\t\"test\"\n\t//42\n}\n"
send({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":doc_uri,"languageId":"zenlang","version":1,"text":source}}})

time.sleep(0.5)
try:
    while True:
        msg = recv()
        if msg is None:
            break
        if msg.get("method") == "textDocument/publishDiagnostics":
            print("=== Diagnostics ===")
            for d in msg.get("params", {}).get("diagnostics", []):
                r = d.get("range", {}).get("start", {})
                print(f"  line={r.get('line')} col={r.get('character')} msg={d.get('message')}")
            print(f"  uri={msg.get('params', {}).get('uri')}")
            break
        else:
            print(f"  other msg: {list(msg.keys())}")
except:
    pass

proc.stdin.close()
proc.stdout.close()
proc.terminate()
