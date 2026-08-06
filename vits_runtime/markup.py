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


def has_chinese_symbols(symbols):
    """判断 symbols 中是否包含中文注音符号（bopomofo）。"""
    for s in symbols:
        if len(s) == 1 and "\u3100" <= s <= "\u312F":
            return True
    return False


def has_kana(text):
    """文本中是否包含平假名/片假名/半角片假名。"""
    for ch in text:
        code = ord(ch)
        if 0x3040 <= code <= 0x30FF or 0x31F0 <= code <= 0x31FF or 0xFF66 <= code <= 0xFF9D:
            return True
    return False


def _char_lang(ch, langs, prefer_ja_for_cjk=False):
    code = ord(ch)
    # 平假名/片假名/半角片假名
    if 0x3040 <= code <= 0x30FF or 0x31F0 <= code <= 0x31FF or 0xFF66 <= code <= 0xFF9D:
        return "ja" if "ja" in langs else None
    # CJK 统一汉字
    if 0x4E00 <= code <= 0x9FFF:
        # 关键启发：只要句子里有假名，就大概率是日语句子，汉字应读日语音
        if prefer_ja_for_cjk and "ja" in langs:
            return "ja"
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


def markup_text(text, langs, prefer_ja_for_cjk=False):
    """把文本按语言切分并用 [XX] 标记包裹。

    标点、数字、空白等无语言属性的字符并入当前片段。
    模型只支持单一语言时直接整体包裹。
    如果文本包含假名，CJK 汉字会被视为日语汉字（避免中日双语模型中
    日语句子里的汉字被误走中文清洗器）。
    """
    if not text:
        return text
    if len(langs) == 1:
        tag = _TAGS[langs[0]]
        return f"[{tag}]{text}[{tag}]"

    # 自动启发：有假名就是日语句子，CJK 按日语处理
    if not prefer_ja_for_cjk and has_kana(text) and "ja" in langs:
        prefer_ja_for_cjk = True

    # 如果以日语为主，把 primary 也调成 ja，避免开头的标点前缀被标成中文
    primary = "ja" if (prefer_ja_for_cjk and "ja" in langs) else langs[0]

    runs = []
    current_lang = primary
    buf = []
    for ch in text:
        lang = _char_lang(ch, langs, prefer_ja_for_cjk) or current_lang
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
