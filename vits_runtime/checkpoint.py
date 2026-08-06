# Adapted from jaywalnut310/vits (MIT License) utils.load_checkpoint
import os
import sys

import torch


def load_checkpoint(checkpoint_path, model):
    assert os.path.isfile(checkpoint_path), f"checkpoint not found: {checkpoint_path}"
    try:
        checkpoint_dict = torch.load(checkpoint_path, map_location="cpu", weights_only=True)
    except Exception:
        # 部分老 checkpoint 含非 tensor 对象，回退到完整反序列化（模型来源需可信）
        checkpoint_dict = torch.load(checkpoint_path, map_location="cpu", weights_only=False)

    if hasattr(model, "module"):
        state_dict = model.module.state_dict()
    else:
        state_dict = model.state_dict()

    saved_state_dict = checkpoint_dict["model"]
    new_state_dict = {}
    for k, v in state_dict.items():
        try:
            new_state_dict[k] = saved_state_dict[k]
        except KeyError:
            print(f"[vits_runtime] {k} is not in the checkpoint", file=sys.stderr)
            new_state_dict[k] = v

    if hasattr(model, "module"):
        model.module.load_state_dict(new_state_dict)
    else:
        model.load_state_dict(new_state_dict)
