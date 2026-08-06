"""按模型支持的语言为文本添加 [JA]/[ZH]/[EN]/[KO] 标记。

vits 系 cleaner（来自 MoeGoe 系 MIT 代码）采用标记式设计：
只有被 [XX]...[XX] 包裹的片段才会走对应语言的转换逻辑，
因此合成前需要按字符语言把文本切分成带标记的片段。
"""

# 各 cleaner 支持的语言（仅列出本运行时内置依赖覆盖的 cleaner）
CLEANER_LANGS = {
    "english_cleaners": ["en"],
    "english_cleaners2": ["en"],
    "japanese_cleaners": ["ja"],
    "japanese_cleaners2": ["ja"],
    "korean_cleaners": ["ko"],
    "chinese_cleaners": ["zh"],
    "zh_ja_mixture_cleaners": ["zh", "ja"],
}

_TAGS = {"ja": "JA", "zh": "ZH", "en": "EN", "ko": "KO"}


def langs_for_cleaners(cleaner_names):
    """从 config.json 的 text_cleaners 推出模型支持的语言列表。"""
    for name in cleaner_names or []:
        if name in CLEANER_LANGS:
            return CLEANER_LANGS[name]
        raise ValueError(
            f"unsupported text cleaner: {name!r}, "
            f"supported: {sorted(CLEANER_LANGS)}"
        )
    # 未声明 cleaner 的模型按日语处理（VITS 模型中最常见）
    return ["ja"]


def _char_lang(ch, langs):
    code = ord(ch)
    # 平假名/片假名/半角片假名
    if 0x3040 <= code <= 0x30FF or 0x31F0 <= code <= 0x31FF or 0xFF66 <= code <= 0xFF9D:
        return "ja" if "ja" in langs else None
    # CJK 统一汉字：双语模型按中文处理，纯日语模型按日语汉字处理
    if 0x4E00 <= code <= 0x9FFF:
        if "zh" in langs:
            return "zh"
        if "ja" in langs:
            return "ja"
        return None
    # 谚文
    if 0xAC00 <= code <= 0xD7AF or 0x1100 <= code <= 0x11FF:
        return "ko" if "ko" in langs else None
    # ASCII 字母
    if ch.isascii() and ch.isalpha():
        return "en" if "en" in langs else None
    return None


def markup_text(text, langs):
    """把文本按语言切分并用 [XX] 标记包裹。

    标点、数字、空白等无语言属性的字符并入当前片段。
    模型只支持单一语言时直接整体包裹。
    """
    if not text:
        return text
    primary = langs[0]
    if len(langs) == 1:
        tag = _TAGS[primary]
        return f"[{tag}]{text}[{tag}]"

    runs = []
    current_lang = primary
    buf = []
    for ch in text:
        lang = _char_lang(ch, langs) or current_lang
        if lang != current_lang and buf:
            runs.append((current_lang, "".join(buf)))
            buf = []
        current_lang = lang
        buf.append(ch)
    if buf:
        runs.append((current_lang, "".join(buf)))

    parts = []
    for lang, segment in runs:
        if not segment.strip():
            parts.append(segment)
            continue
        tag = _TAGS[lang]
        parts.append(f"[{tag}]{segment}[{tag}]")
    return " ".join(parts)
