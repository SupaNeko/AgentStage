# VITS 合成器封装，推理流程参考 vits-simple-api（vits/vits.py，MoeGoe 系 MIT 代码）
import glob
import json
import os
import re
import sys

import numpy as np
import torch
from torch import LongTensor, no_grad

from checkpoint import load_checkpoint
from hparams import get_hparams_from_file
from markup import has_chinese_symbols, langs_for_cleaners, markup_text
from vits import commons
from vits.models import SynthesizerTrn
from vits.text import text_to_sequence
from vits.text import cleaners as cleaners_module

# 默认合成参数（与 MoeGoe 默认值一致）
DEFAULT_NOISE = 0.667
DEFAULT_NOISEW = 0.8

# 句末标点，长文本按此切分后分段合成再拼接
_SENTENCE_END = re.compile(r"([^。！？!?…\.\n]+[。！？!?…\.\n]*)")
# 段内二次切分（超长句）
_CLAUSE_END = re.compile(r"([^，、,；;：:]+[，、,；;：:]*)"
)

MAX_SEGMENT_CHARS = 120


def _log(msg):
    print(f"[vits_runtime] {msg}", file=sys.stderr, flush=True)


def split_sentences(text):
    """把长文本切成适合逐段合成的短句列表。"""
    text = re.sub(r"\s+", " ", text).strip()
    if not text:
        return []
    sentences = []
    for piece in _SENTENCE_END.findall(text):
        piece = piece.strip()
        if not piece:
            continue
        if len(piece) <= MAX_SEGMENT_CHARS:
            sentences.append(piece)
        else:
            buf = ""
            for clause in _CLAUSE_END.findall(piece):
                if len(buf) + len(clause) > MAX_SEGMENT_CHARS and buf:
                    sentences.append(buf)
                    buf = clause
                else:
                    buf += clause
            if buf:
                sentences.append(buf)
    return sentences


def parse_emotion_params(raw):
    """emotion_params 为 JSON 字符串，支持 {"noise": 0.667, "noisew": 0.8}。"""
    noise, noisew = DEFAULT_NOISE, DEFAULT_NOISEW
    if not raw:
        return noise, noisew
    try:
        params = json.loads(raw)
        if not isinstance(params, dict):
            return noise, noisew
        noise = float(params.get("noise", noise))
        noisew = float(params.get("noisew", noisew))
    except (ValueError, TypeError):
        pass
    return noise, noisew


class VitsSynthesizer:
    """加载单个 VITS 模型目录（config.json + 唯一 .pth），执行文本合成。"""

    def __init__(self, model_dir, device="cpu"):
        config_path = os.path.join(model_dir, "config.json")
        if not os.path.isfile(config_path):
            raise FileNotFoundError(f"config.json not found in {model_dir}")
        pth_files = glob.glob(os.path.join(model_dir, "*.pth"))
        if len(pth_files) == 0:
            raise FileNotFoundError(f"no .pth checkpoint found in {model_dir}")
        if len(pth_files) > 1:
            raise RuntimeError(f"multiple .pth files in {model_dir}, keep exactly one")

        self.hps = get_hparams_from_file(config_path)
        self.n_speakers = getattr(self.hps.data, "n_speakers", 0)
        self.n_symbols = len(getattr(self.hps, "symbols", []))
        speakers = getattr(self.hps, "speakers", ["0"])
        if not isinstance(speakers, list):
            # 兼容 speakers 为 {name: id} 字典的 config
            speakers = [k for k, _ in sorted(speakers.items(), key=lambda x: x[1])]
        self.speakers = speakers
        self.sampling_rate = self.hps.data.sampling_rate
        self.text_cleaners = getattr(self.hps.data, "text_cleaners", [])
        self.langs = langs_for_cleaners(self.text_cleaners)

        # 关键修正：很多中日混合模型 config 写 zh_ja，但 weights 没学中文符号
        # 若 symbols 里没有中文注音，则降级为纯日语模型，避免汉字被误走中文清洗
        if "zh" in self.langs and not has_chinese_symbols(self.hps.symbols):
            _log(
                f"model claims zh_ja support but has no chinese bopomofo symbols; "
                f"downgrading languages from {self.langs} to ['ja']"
            )
            self.langs = ["ja"]

        self.device = torch.device(device)

        self.net_g = SynthesizerTrn(
            self.n_symbols,
            self.hps.data.filter_length // 2 + 1,
            self.hps.train.segment_size // self.hps.data.hop_length,
            n_speakers=self.n_speakers,
            **self.hps.model,
        )
        self.net_g.eval()
        load_checkpoint(pth_files[0], self.net_g)
        self.net_g.to(self.device)

    def resolve_speaker_id(self, speaker):
        """speaker 可以是说话人名称或数字下标，缺省取 0。"""
        if speaker is None or speaker == "":
            return 0
        if isinstance(speaker, str) and speaker in self.speakers:
            return self.speakers.index(speaker)
        try:
            sid = int(speaker)
        except (ValueError, TypeError):
            raise ValueError(f"unknown speaker: {speaker}, available: {self.speakers}")
        if self.n_speakers > 0 and not (0 <= sid < self.n_speakers):
            raise ValueError(f"speaker id {sid} out of range 0..{self.n_speakers - 1}")
        return sid

    def get_text_sequence(self, text):
        marked = markup_text(text, self.langs)
        _log(f"get_text_sequence: text={text!r} marked={marked!r} langs={self.langs} cleaners={self.text_cleaners}")
        # 手动调用 cleaner 以观察清洗结果（debug）
        cleaned = marked
        for name in self.text_cleaners:
            cleaner = getattr(cleaners_module, name)
            cleaned = cleaner(cleaned)
        _log(f"get_text_sequence: cleaned={cleaned!r}")
        text_norm = text_to_sequence(marked, self.hps.symbols, self.text_cleaners)
        _log(f"get_text_sequence: text_norm length={len(text_norm)} ids={text_norm}")
        if len(text_norm) < 3:
            raise RuntimeError(
                f"text produced only {len(text_norm)} symbols after cleaning; "
                f"model likely does not support the input language. "
                f"marked='{marked}' cleaned='{cleaned}' symbols={len(self.hps.symbols)}"
            )
        _log(
            f"text='{text}' -> marked='{marked}' -> cleaned='{cleaned}' -> sequence_len={len(text_norm)}"
        )
        if getattr(self.hps.data, "add_blank", False):
            text_norm = commons.intersperse(text_norm, 0)
        return LongTensor(text_norm)

    def infer_segment(self, text, speaker_id, noise, noisew, length):
        stn_tst = self.get_text_sequence(text)
        sid = LongTensor([speaker_id]).to(self.device)
        with no_grad():
            x_tst = stn_tst.unsqueeze(0).to(self.device)
            x_tst_lengths = LongTensor([stn_tst.size(0)]).to(self.device)
            audio = self.net_g.infer(
                x=x_tst,
                x_lengths=x_tst_lengths,
                sid=sid,
                noise_scale=noise,
                noise_scale_w=noisew,
                length_scale=length,
            )[0][0, 0].data.float().cpu().numpy()
        return audio

    def synthesize(self, text, speaker=None, speed=1.0, emotion_params=None):
        """合成整段文本，返回 (float32 波形, 采样率)。"""
        sentences = split_sentences(text)
        if not sentences:
            raise ValueError("empty text after normalization")
        speaker_id = self.resolve_speaker_id(speaker)
        noise, noisew = parse_emotion_params(emotion_params)
        # speed 与 length_scale 成反比：语速 2.0x => length_scale 0.5
        speed = float(speed) if speed else 1.0
        speed = min(max(speed, 0.5), 2.0)
        length = 1.0 / speed

        _log(
            f"synthesize: sentences={len(sentences)} speaker_id={speaker_id} "
            f"speed={speed} length_scale={length:.3f} noise={noise:.3f} noisew={noisew:.3f}"
        )

        brk = np.zeros(int(0.3 * self.sampling_rate), dtype=np.float32)
        audios = []
        for i, sentence in enumerate(sentences):
            audios.append(self.infer_segment(sentence, speaker_id, noise, noisew, length))
            if i < len(sentences) - 1:
                audios.append(brk)
        return np.concatenate(audios, axis=0), self.sampling_rate
