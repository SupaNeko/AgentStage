"""按模型配置的目标语言为文本添加 [JA]/[ZH]/[EN]/[KO] 标记。

vits 系 cleaner（来自 MoeGoe 系 MIT 代码）采用标记式设计：
只有被 [XX]...[XX] 包裹的片段才会走对应语言的转换逻辑。

本应用的设计是：语音模型在角色配置中已经指定了目标语言，翻译步骤也保证
输入文本就是目标语言。因此我们不按字符再次判断语言，而是直接整段文本
包进目标语言对应的标签，交给对应 cleaner 处理。
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


def lang_for_cleaner(cleaner_name):
    """从单个 cleaner 名称推断默认目标语言，未识别时默认日语。"""
    return CLEANER_LANGS.get(cleaner_name, ["ja"])[0]


def supported_langs_for_cleaner(cleaner_name):
    """返回该 cleaner 支持的语言代码列表。"""
    return CLEANER_LANGS.get(cleaner_name, ["ja"])


def markup_text(text, target_lang):
    """把整段文本用目标语言标签包裹。

    Args:
        text: 待合成的目标语言文本（翻译后或原本就是该语言）。
        target_lang: 目标语言代码，可选 ja/zh/en/ko。

    Returns:
        带 [XX]...[XX] 标签的文本，供对应 cleaner 消费。
    """
    if not text:
        return text
    tag = _TAGS.get(target_lang, "JA")
    # 去除首尾空白后整体包裹，避免空白被 cleaner 当作额外片段
    return f"[{tag}]{text.strip()}[{tag}]"
