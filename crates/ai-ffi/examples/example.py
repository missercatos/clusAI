"""
Python example for ai-ffi.

Run: LD_LIBRARY_PATH=../../target/release python3 example.py
"""

import ctypes
import json
import os
import sys
import time

# Locate the shared library
_lib_dir = os.path.join(os.path.dirname(__file__), "..", "..", "target", "release")
_lib_path = os.path.join(_lib_dir, "libai_ffi.so")
if not os.path.exists(_lib_path):
    _lib_path = os.path.join(_lib_dir, "libai_ffi.dylib")

_lib = ctypes.CDLL(_lib_path)

# Type signatures
_lib.ai_init.argtypes = [ctypes.c_char_p]
_lib.ai_init.restype = ctypes.c_void_p

_lib.ai_think.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
_lib.ai_think.restype = ctypes.c_int

_lib.ai_poll.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]
_lib.ai_poll.restype = ctypes.c_int

_lib.ai_done.argtypes = [ctypes.c_void_p]
_lib.ai_done.restype = ctypes.c_int

_lib.ai_free.argtypes = [ctypes.c_void_p]
_lib.ai_free.restype = None

_lib.ai_version.restype = ctypes.c_char_p


class AiHandle:
    def __init__(self, config_path: str = "./.clusai.toml"):
        self._h = _lib.ai_init(config_path.encode())
        if not self._h:
            raise RuntimeError("ai_init failed — check stderr")

    def think(self, prompt: str) -> "AiStream":
        rc = _lib.ai_think(self._h, prompt.encode())
        if rc != 0:
            raise RuntimeError("ai_think failed")
        return AiStream(self._h)

    def close(self):
        if self._h:
            _lib.ai_free(self._h)
            self._h = None

    def __del__(self):
        self.close()


class AiStream:
    def __init__(self, handle):
        self._h = handle

    def __iter__(self):
        return self

    def __next__(self):
        if _lib.ai_done(self._h):
            raise StopIteration
        buf = ctypes.create_string_buffer(8192)
        n = _lib.ai_poll(self._h, buf, 8192)
        if n <= 0:
            time.sleep(0.01)
            return self.__next__()
        return json.loads(buf.value[:n])


def main():
    config = sys.argv[1] if len(sys.argv) > 1 else "./.clusai.toml"

    print(f"ai-core version: {_lib.ai_version().decode()}")

    ai = AiHandle(config)
    try:
        for event in ai.think("Introduce yourself briefly."):
            t = event.get("type", "?")
            if t == "text_delta":
                print(event["content"], end="", flush=True)
            elif t == "tool_call_start":
                print(f"\n[TOOL] {event['tool_name']}")
            elif t == "tool_call_end":
                print(f"\n[TOOL] {event['tool_name']} => {'OK' if event['succeeded'] else 'FAIL'}")
            elif t == "agent_finished":
                print("\n--- done ---")
            elif t == "error":
                print(f"\n[ERROR] {event['message']}")
    finally:
        ai.close()


if __name__ == "__main__":
    main()
