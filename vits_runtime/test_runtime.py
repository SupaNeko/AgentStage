"""VITS runtime 纯逻辑单元测试（不依赖 torch，stub 掉模型相关导入）。

运行：.venv/Scripts/python.exe test_runtime.py
"""
import contextlib
import os
import sys
import types
import unittest

# 在 import synthesizer 之前 stub 掉 torch 与模型侧依赖
torch_stub = types.ModuleType("torch")
torch_stub.LongTensor = lambda x: x
torch_stub.no_grad = contextlib.nullcontext
torch_stub.device = lambda d: d
sys.modules["torch"] = torch_stub

for name in ["checkpoint", "hparams", "vits", "vits.commons", "vits.models", "vits.text", "vits.text.cleaners"]:
    sys.modules[name] = types.ModuleType(name)
sys.modules["checkpoint"].load_checkpoint = None
sys.modules["hparams"].get_hparams_from_file = None
sys.modules["vits.models"].SynthesizerTrn = None
sys.modules["vits.text"].text_to_sequence = None
sys.modules["vits.commons"].intersperse = None

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import synthesizer  # noqa: E402
import markup  # noqa: E402


class TestSplitSentences(unittest.TestCase):
    def test_chinese_sentences(self):
        self.assertEqual(
            synthesizer.split_sentences("你好。今天天气不错！我们去玩吧？"),
            ["你好。", "今天天气不错！", "我们去玩吧？"],
        )

    def test_japanese_sentences(self):
        self.assertEqual(
            synthesizer.split_sentences("こんにちは。元気ですか。"),
            ["こんにちは。", "元気ですか。"],
        )

    def test_english_sentences(self):
        self.assertEqual(
            synthesizer.split_sentences("Hello world. How are you?"),
            ["Hello world.", "How are you?"],
        )

    def test_empty_and_blank(self):
        self.assertEqual(synthesizer.split_sentences(""), [])
        self.assertEqual(synthesizer.split_sentences("   \n  "), [])

    def test_whitespace_normalized(self):
        self.assertEqual(
            synthesizer.split_sentences("你好。\n\n  世界。"),
            ["你好。", "世界。"],
        )

    def test_long_sentence_split_by_clause(self):
        long_text = "这是一个很长的句子，" * 20 + "最后结束。"
        parts = synthesizer.split_sentences(long_text)
        self.assertGreater(len(parts), 1)
        for part in parts:
            self.assertLessEqual(len(part), synthesizer.MAX_SEGMENT_CHARS)
        # 拼接后内容不丢字（仅末尾标点可能被合并）
        self.assertEqual("".join(parts), long_text)

    def test_no_trailing_punctuation(self):
        self.assertEqual(synthesizer.split_sentences("没有结尾标点"), ["没有结尾标点"])


class TestParseEmotionParams(unittest.TestCase):
    def test_defaults(self):
        self.assertEqual(synthesizer.parse_emotion_params(None), (0.667, 0.8))
        self.assertEqual(synthesizer.parse_emotion_params(""), (0.667, 0.8))

    def test_valid_json(self):
        self.assertEqual(synthesizer.parse_emotion_params('{"noise": 0.9, "noisew": 0.1}'), (0.9, 0.1))

    def test_partial_json(self):
        self.assertEqual(synthesizer.parse_emotion_params('{"noise": 0.5}'), (0.5, 0.8))

    def test_invalid_input_falls_back(self):
        self.assertEqual(synthesizer.parse_emotion_params("not json"), (0.667, 0.8))
        self.assertEqual(synthesizer.parse_emotion_params("happy"), (0.667, 0.8))
        self.assertEqual(synthesizer.parse_emotion_params("[1,2]"), (0.667, 0.8))


class TestMarkup(unittest.TestCase):
    def test_wraps_japanese(self):
        self.assertEqual(
            markup.markup_text("こんにちは。", "ja"),
            "[JA]こんにちは。[JA]",
        )

    def test_wraps_chinese(self):
        self.assertEqual(
            markup.markup_text("你好，世界。", "zh"),
            "[ZH]你好，世界。[ZH]",
        )

    def test_wraps_english(self):
        self.assertEqual(
            markup.markup_text("Hello world.", "en"),
            "[EN]Hello world.[EN]",
        )

    def test_wraps_korean(self):
        self.assertEqual(
            markup.markup_text("안녕하세요.", "ko"),
            "[KO]안녕하세요.[KO]",
        )

    def test_whitespace_stripped(self):
        self.assertEqual(
            markup.markup_text("  こんにちは。  \n", "ja"),
            "[JA]こんにちは。[JA]",
        )

    def test_lang_for_cleaner(self):
        self.assertEqual(markup.lang_for_cleaner("japanese_cleaners"), "ja")
        self.assertEqual(markup.lang_for_cleaner("chinese_cleaners"), "zh")
        self.assertEqual(markup.lang_for_cleaner("english_cleaners"), "en")
        self.assertEqual(markup.lang_for_cleaner("zh_ja_mixture_cleaners"), "zh")
        self.assertEqual(markup.lang_for_cleaner(""), "ja")
        self.assertEqual(markup.lang_for_cleaner("unknown_cleaners"), "ja")


class TestProtocolHandling(unittest.TestCase):
    """main.py 的请求分发逻辑（generate 之外的 action 不需要模型）。"""

    def test_ping(self):
        import main
        self.assertEqual(main.handle_request({"action": "ping"}), {"success": True, "message": "pong"})

    def test_unknown_action(self):
        import main
        resp = main.handle_request({"action": "nope"})
        self.assertFalse(resp["success"])
        self.assertIn("unknown action", resp["message"])

    def test_generate_missing_fields(self):
        import main
        self.assertFalse(main.handle_request({"action": "generate", "text": "  "})["success"])
        self.assertFalse(
            main.handle_request({"action": "generate", "text": "hi", "output_path": "x.wav"})["success"]
        )
        self.assertFalse(
            main.handle_request({"action": "generate", "text": "hi", "model_path": "m"})["success"]
        )


if __name__ == "__main__":
    unittest.main(verbosity=2)
