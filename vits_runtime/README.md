# VITS Runtime（AgentStage 语音合成独立运行时）

基于开源项目二次开发的 VITS 推理运行时，以独立进程方式被 AgentStage 主程序启动，
通过 **stdin/stdout 逐行 JSON** 通信（无端口、无防火墙弹窗、随主程序退出自动回收）。

## 代码来源与许可证

| 路径 | 来源 | 许可证 |
|------|------|--------|
| `vits/` | [vits-simple-api](https://github.com/Artrajz/vits-simple-api) 的 `vits/` 目录（MoeGoe 系推理核心） | MIT（见 `vits/LICENSE`） |
| `hparams.py` / `checkpoint.py` | 改编自 [jaywalnut310/vits](https://github.com/jaywalnut310/vits) `utils` | MIT |
| `main.py` / `synthesizer.py` | 本项目原创 | 与 AgentStage 主项目一致 |

> 注意：仅复用了 MIT 许可的推理核心；vits-simple-api 上层的 FastAPI 服务代码（AGPL）未被引入。
> 相比上游做了如下适配：移除对 `config` 全局配置的依赖、词典路径改为模块相对路径、
> 去掉日语词典自动下载、espeak/phonemizer 改为按需懒加载、日语依赖换成
> `pyopenjtalk-plus`（pyopenjtalk-prebuilt 不支持 Python 3.12+）。

## 语言标记

vits 系 cleaner 采用标记式设计：只有 `[JA]...[JA]` / `[ZH]...[ZH]` 等标签内的文本
才会走对应语言的转换。运行时根据模型 `config.json` 的 `text_cleaners` 自动推导
支持的语言，并按字符类型（假名→JA、汉字→ZH/JA、ASCII→EN）自动切分打标（`markup.py`）。

目前内置依赖覆盖的 cleaner：`japanese_cleaners(2)`、`chinese_cleaners`、
`zh_ja_mixture_cleaners`、`korean_cleaners`、`english_cleaners(2)`（英语 IPA 系需额外装依赖）。
`cjke_/cje_` 等依赖 espeak 的 cleaner 会明确报错。

## 模型目录约定

用户把模型放到主程序数据目录 `data/vits_models/<模型名>/` 下：

```
data/vits_models/
  my_model/
    config.json   # VITS 训练配置（hparams）
    xxx.pth       # 权重文件，有且仅有一个
```

`config.json` 中 `speakers` 为数组或 `{名称: id}` 字典均可识别；
`data.text_cleaners` 决定文本清洗方式，对应语言依赖见 `requirements.txt`。

## 通信协议

- 启动完成后输出一行：`{"ready": true, "version": "1.0.0"}`
- 之后每行一个请求，每请求对应一行响应；日志写 stderr

请求：
```json
{"action": "generate", "text": "こんにちは", "model_path": "D:\\...\\my_model", "speaker_id": "綾地寧々", "emotion_params": "{\"noise\": 0.667, \"noisew\": 0.8}", "speed": 1.0, "output_path": "D:\\...\\out.wav"}
```

字段说明：
- `speaker_id`：说话人名称或数字下标，可空（默认 0）
- `emotion_params`：JSON 字符串，`noise`（情感变化幅度）/ `noisew`（音素时长随机性），可空
- `speed`：语速 0.5~2.0，内部换算为 `length_scale = 1 / speed`

响应：
```json
{"success": true, "output_path": "D:\\...\\out.wav", "duration_ms": 2350}
```

## 本地调试

```powershell
cd vits_runtime
python -m venv .venv
.\.venv\Scripts\pip install torch --index-url https://download.pytorch.org/whl/cpu
.\.venv\Scripts\pip install -r requirements.txt
.\.venv\Scripts\python.exe main.py
# 等待输出 {"ready": true, ...} 后，粘贴一行请求 JSON 回车即可
```

## 测试

```powershell
# 纯逻辑单测（分句/情感参数/语言标记/协议分发，不依赖 torch）
.\.venv\Scripts\python.exe -X utf8 test_runtime.py
# 端到端冒烟测试（合成随机权重玩具模型，走完整 stdin/stdout 协议）
.\.venv\Scripts\python.exe -X utf8 test_smoke.py
```

## 打包为独立 exe（PyInstaller）

```powershell
cd vits_runtime
pip install pyinstaller
pyinstaller --onefile --name vits_runtime `
  --collect-all pyopenjtalk `
  --collect-all jieba `
  --add-data "vits/text/jieba/dict.txt;vits/text/jieba" `
  main.py
# 产物：dist/vits_runtime.exe
```

打包产物交付时放置到主程序数据目录：

```
data/vits_runtime/vits_runtime.exe
```

## 当前限制

- 仅支持标准 VITS（`.pth` + `config.json`）；Bert-VITS2 / GPT-SoVITS / W2V2-VITS 未接入
- `bert_embedding` 类模型（需 prosody bert 目录）暂不支持
- CPU 推理；长文本自动分段合成（段间 0.3s 停顿）
