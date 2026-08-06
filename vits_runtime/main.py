"""VITS 独立推理运行时入口。

通过 stdin/stdout 逐行 JSON-RPC 与 AgentStage 主程序通信：
- 启动完成后输出一行就绪信号：{"ready": true, "version": "..."}
- 之后每行一个请求 JSON，每请求对应一行响应 JSON
- 日志一律写 stderr，保持 stdout 纯净

请求（generate）：
{"action": "generate", "text": "...", "model_path": "<模型目录>",
 "speaker_id": "名称或下标，可空", "emotion_params": "{\"noise\":0.667}", "speed": 1.0,
 "output_path": "<输出 wav 路径>"}

响应：
{"success": true, "output_path": "...", "duration_ms": 1234}
{"success": false, "message": "错误描述"}
"""
import json
import sys
import time
import wave
import contextlib

import numpy as np

VERSION = "1.0.0"

# model_path -> VitsSynthesizer，模型常驻内存避免重复加载
_models = {}


def log(msg):
    print(f"[vits_runtime] {msg}", file=sys.stderr, flush=True)


def get_synthesizer(model_path):
    if model_path not in _models:
        from synthesizer import VitsSynthesizer

        log(f"loading model: {model_path}")
        start = time.time()
        _models[model_path] = VitsSynthesizer(model_path)
        log(f"model loaded in {time.time() - start:.1f}s")
    return _models[model_path]


def write_wav(path, audio, sampling_rate):
    pcm = (np.clip(audio, -1.0, 1.0) * 32767).astype(np.int16)
    with wave.open(path, "wb") as wf:
        wf.setnchannels(1)
        wf.setsampwidth(2)
        wf.setframerate(int(sampling_rate))
        wf.writeframes(pcm.tobytes())


def handle_generate(req):
    text = req.get("text") or ""
    model_path = req.get("model_path")
    output_path = req.get("output_path")
    if not text.strip():
        return {"success": False, "message": "empty text"}
    if not model_path:
        return {"success": False, "message": "missing model_path"}
    if not output_path:
        return {"success": False, "message": "missing output_path"}

    synth = get_synthesizer(model_path)
    audio, sr = synth.synthesize(
        text,
        speaker=req.get("speaker_id"),
        speed=req.get("speed") or 1.0,
        emotion_params=req.get("emotion_params"),
    )
    write_wav(output_path, audio, sr)
    duration_ms = int(len(audio) / sr * 1000)
    return {"success": True, "output_path": output_path, "duration_ms": duration_ms}


def handle_request(req):
    action = req.get("action")
    if action == "generate":
        return handle_generate(req)
    if action == "ping":
        return {"success": True, "message": "pong"}
    return {"success": False, "message": f"unknown action: {action}"}


def main():
    # 就绪信号必须在任何推理之前输出，主程序以此判断启动成功
    print(json.dumps({"ready": True, "version": VERSION}), flush=True)

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        # 第三方库（如 pyopenjtalk-plus）可能向 stdout 打印警告，污染协议通道，
        # 请求处理期间把 Python 层 stdout 重定向到 stderr，保证响应是 stdout 唯一内容。
        with contextlib.redirect_stdout(sys.stderr):
            try:
                req = json.loads(line)
            except json.JSONDecodeError as e:
                resp = {"success": False, "message": f"invalid json: {e}"}
            else:
                try:
                    resp = handle_request(req)
                except Exception as e:
                    log(f"request failed: {e}")
                    resp = {"success": False, "message": str(e)}
        print(json.dumps(resp, ensure_ascii=False), flush=True)


if __name__ == "__main__":
    main()
