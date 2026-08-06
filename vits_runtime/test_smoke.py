"""端到端冒烟测试：合成一个随机权重的玩具 VITS 模型，通过 stdin/stdout 协议驱动 main.py。

需要完整依赖（torch 等）。运行：.venv/Scripts/python.exe test_smoke.py
输出音频无意义（随机权重），仅验证 加载模型→清洗→推理→写 WAV→协议响应 全链路可用。
"""
import json
import os
import subprocess
import sys
import tempfile

import torch

from hparams import HParams


def make_fake_model(model_dir):
    os.makedirs(model_dir, exist_ok=True)

    symbols = sorted(set(
        list("_-~!\\\"'(),.:;? ")
        + list("abcdefghijklmnopqrstuvwxyz")
        + list("ABCDEFGHIJKLMNOPQRSTUVWXYZ")
        + list("0123456789")
        + list("↑↓ʦʧɯ^ˈˌ…。、！？，")
    ))

    config = {
        "train": {"segment_size": 8192},
        "data": {
            "sampling_rate": 22050,
            "filter_length": 1024,
            "hop_length": 256,
            "win_length": 1024,
            "n_speakers": 0,
            "text_cleaners": ["japanese_cleaners"],
            "add_blank": True,
        },
        "model": {
            "inter_channels": 64,
            "hidden_channels": 64,
            "filter_channels": 128,
            "n_heads": 2,
            "n_layers": 2,
            "kernel_size": 3,
            "p_dropout": 0.0,
            "resblock": "1",
            "resblock_kernel_sizes": [3, 7, 11],
            "resblock_dilation_sizes": [[1, 3, 5], [1, 3, 5], [1, 3, 5]],
            "upsample_rates": [8, 8, 2, 2],
            "upsample_initial_channel": 128,
            "upsample_kernel_sizes": [16, 16, 4, 4],
            "n_layers_q": 2,
            "use_spectral_norm": False,
            "gin_channels": 0,
        },
        "symbols": symbols,
        "speakers": ["测试说话人"],
    }

    with open(os.path.join(model_dir, "config.json"), "w", encoding="utf-8") as f:
        json.dump(config, f, ensure_ascii=False)

    # 用同一套 hparams 构造网络并保存随机权重
    from synthesizer import VitsSynthesizer  # noqa: F401  （确认导入链完整）
    from vits.models import SynthesizerTrn

    hps = HParams(**config)
    net = SynthesizerTrn(
        len(symbols),
        hps.data.filter_length // 2 + 1,
        hps.train.segment_size // hps.data.hop_length,
        n_speakers=0,
        **hps.model,
    )
    torch.save({"model": net.state_dict()}, os.path.join(model_dir, "fake.pth"))


def main():
    tmp = tempfile.mkdtemp(prefix="vits_smoke_")
    model_dir = os.path.join(tmp, "fake_model")
    out_path = os.path.join(tmp, "out.wav")
    make_fake_model(model_dir)
    print(f"fake model at {model_dir}")

    runtime_dir = os.path.dirname(os.path.abspath(__file__))
    env = {**os.environ, "PYTHONIOENCODING": "utf-8"}
    proc = subprocess.Popen(
        [sys.executable, os.path.join(runtime_dir, "main.py")],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        encoding="utf-8",
        errors="replace",
        cwd=runtime_dir,
        env=env,
    )

    ready = proc.stdout.readline()
    ready_obj = json.loads(ready)
    assert ready_obj.get("ready"), f"not ready: {ready}"
    print(f"ready signal OK: {ready.strip()}")

    req = {
        "action": "generate",
        "text": "こんにちは。今日はいい天気ですね。",
        "model_path": model_dir,
        "speaker_id": None,
        "emotion_params": '{"noise": 0.5}',
        "speed": 1.2,
        "target_language": "ja",
        "output_path": out_path,
    }
    proc.stdin.write(json.dumps(req, ensure_ascii=False) + "\n")
    proc.stdin.flush()
    resp = json.loads(proc.stdout.readline())
    print(f"response: {resp}")
    assert resp.get("success"), f"generate failed: {resp}"
    assert resp["output_path"] == out_path
    assert resp["duration_ms"] > 0
    assert os.path.isfile(out_path) and os.path.getsize(out_path) > 100

    # 第二次请求复用已加载模型
    proc.stdin.write(json.dumps({**req, "text": "さようなら。"}, ensure_ascii=False) + "\n")
    proc.stdin.flush()
    resp2 = json.loads(proc.stdout.readline())
    assert resp2.get("success"), f"second generate failed: {resp2}"

    # 错误路径：不存在的模型
    proc.stdin.write(json.dumps({**req, "model_path": os.path.join(tmp, "nope")}, ensure_ascii=False) + "\n")
    proc.stdin.flush()
    resp3 = json.loads(proc.stdout.readline())
    assert not resp3.get("success"), "expected failure for missing model"

    proc.stdin.close()
    proc.wait(timeout=30)
    err = proc.stderr.read()
    if err.strip():
        print(f"--- child stderr ---\n{err}")
    print("SMOKE TEST PASSED")


if __name__ == "__main__":
    main()
