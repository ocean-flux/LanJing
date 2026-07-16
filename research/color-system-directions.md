# 览境生产色系研究方向（不实施）

日期：2026-07-17  
范围：研究 + 方向草案。**未改** PRODUCT/DESIGN/CSS。  
约束：product register · Restrained · L1 角色名稳定 · L2 只重绑 hex · 壳=安静精密 · 沉浸仅内容面。

## 1. 现状盘点

### 生产（唯一默认包）

- 名：纸灯精密 `paper-lantern-precision`
- canvas `#f5f4f1` · ink `#1f1e1c` · lantern `#c2683a` · strong `#9a4e2c`
- dark canvas `#141417`（偏冷炭，暖墨对比）
- 策略：Restrained；lantern ≤~10%

### 原型（dev only，三套）

| id     | 名         | accent 感        |
| ------ | ---------- | ---------------- |
| signal | 冷银朱红   | 暖橙红，近现生产 |
| tide   | 墨青冰蓝   | 冷青，原型默认   |
| volt   | 黑白电光黄 | 高对比实验       |

### 架构（正确，应保留）

```
L0 light|dark|system
L1 roles: canvas/ink/lantern/reader-*/media-void/status  ← 名永不改
L2 AppearancePack 重绑 hex                              ← 气质换皮处
L3 mode atmosphere 仅内容面
Reader prefs 独立于 L0
```

## 2. 设计诊断（impeccable product + colorize）

**对的**

- Restrained + 语义角色 + 壳/内容分裂
- dark 不整页橙雾
- status 与 lantern 分离

**弱 / 可重做理由**

1. **暖中性 + 暖灯同向** — colorize 警告：mood 应在 brand，勿 canvas 与 primary 双暖；现 canvas 暖纸 + copper 同暖 → 易读成「AI cream + copper」。
2. **铜灯与 signal 几乎同族** — 原型「冷银朱红」未真正拉开；用户记的「别的色」多半是 **tide**。
3. **暗色 canvas 冷、灯暖** — 有意对比，但选中 `lantern-soft` 在冷炭上有时发脏。
4. **阅读器纸 `#f4efe4` 与壳 canvas 过近** — 进阅读器「换气」不够。
5. **「纸灯」叙事** — 若产品更偏精密工具而非纸灯意象，名与 hex 都可换，**角色名 `lantern` 可保留作语义**（主行动/选择），不必绑铜。

## 3. 场景句（定策略用）

> 深夜本地工作台：长会话读/听/翻库；壳像精密仪器框，内容面自带纸/空/舞台；无 SaaS 仪表盘、无霓虹播放器壳。

策略锁定：**Restrained**。  
bg 原则：优先 **近纯中性**（chroma 极低）或 **向 brand 极轻 tint**，避免 cream 默认。

## 4. 四条生产方向（L2 包草案）

角色名不变；下表为 **建议 hex 方向**（实施前再校准对比度）。

### A · 墨砚精密（推荐默认候选）

**一句话：** 近无彩工作台 + 深青灰 accent — 像潮水灯，不是铜灯。  
**从哪来：** 原型 tide 气质产品化；比 tide 更克制、更工具。  
**Light**

- canvas ≈ `#f4f5f5`（chroma≈0）
- ink ≈ `#171a1b`
- lantern ≈ `#2a6f7a` · strong ≈ `#1d5560` · soft 低 alpha
- reader-canvas ≈ `#eef3f2` 或独立暖纸 `#f3efe6`（阅读可仍偏纸）
  **Dark**
- canvas ≈ `#0e1214`
- ink ≈ `#e6eceb`
- lantern ≈ `#5fa8b4`（暗上提亮）
  **气质：** 冷静、连续、local-first 实验室。  
  **风险：** 与「纸灯」命名冲突 → L2 包改名，如 `inkstone-precision`。  
  **a11y：** strong 作按钮字 on-lantern 需浅字；校验 ≥4.5。

### B · 霜铜修订（最小变更）

**一句话：** 保留铜叙事，修「双暖」病。  
**Light**

- canvas ≈ `#f7f7f6` 或 `#fafafa`（降暖）
- lantern 略降 chroma / 略偏红褐 `#a85a38`–`#b35c36`
- reader 拉开：`#f2ebe0`
  **Dark**
- canvas 保持冷炭；soft 用更低 alpha 或 hue 对齐 canvas
  **气质：** 仍「纸灯」，但更干净。  
  **风险：** 用户已嫌铜则不够。

### C · 冷银朱（signal 产品化）

**一句话：** 中性银壳 + 朱红行动，非奶油铜。  
**Light**

- canvas ≈ `#f2f2f0`（微冷）
- lantern ≈ `#c45a3c` · strong ≈ `#9a3f2a`
  **Dark**
- canvas ≈ `#121316`
- lantern 提亮环，避免泥橙
  **气质：** 信号感、略锋利。  
  **风险：** 与危险色 danger 更近，状态色要拉开。

### D · 双温分域（结构向，非换主色）

**一句话：** 壳中性无彩；**仅 reader/media** 用纸暖或 void 冷。

- 壳 lantern 用 A 或 C
- reader-\* 固定暖纸族，与壳脱钩（已有 prefs，再强化默认差）  
  **气质：** Adaptive Frame 最干净。  
  **可与 A/B/C 叠加。**

## 5. 明确不推荐

| 方向                      | 原因                    |
| ------------------------- | ----------------------- |
| volt 电光黄作默认         | 产品 hype，反 calm      |
| palette.mjs 随机玫红 seed | 与 brief 无关           |
| Committed/Drenched 壳     | 壳会抢媒体              |
| 多 L2 包 UI 现上          | PRODUCT 已 out of scope |
| 改 L1 角色名              | 破坏契约                |

## 6. 落地时改动面（实施清单，现不写代码）

1. `DESIGN.md` frontmatter + 叙述 + pack 名
2. `.impeccable/design.json`
3. `src/index.css` `:root` / `.dark` 与 focus-ring
4. `theme.svelte.ts` 默认 pack id（若改名）
5. 测试里写死 pack id 的断言
6. **不**把 `prototype-palette-*` 接进生产组件
7. 截图验收：壳 rail 选中、主按钮、搜索 CTA、reader、danger/warning

## 7. 建议决策顺序

1. 先定 **主色族**：青灰（A）vs 修订铜（B）vs 朱红（C）
2. 再定 **canvas**：近纯中性（推荐）vs 极轻 brand tint
3. 叠加 **D**：reader 与壳温差
4. 包命名：保留「纸灯精密」仅当选 B；选 A 则新名
5. 一次 token PR；不做多包切换 UI

## 8. 研究结论

- 当前实现 **忠于已文档化的纸灯精密**，不是 bug。
- 用户「不是这个色」合理来源：**原型 tide 默认** vs **生产铜灯**，或对暖奶油壳的厌倦。
- 最贴 PRODUCT（calm / craft / continuous / quiet shell）的默认候选：**A 墨砚精密**（tide 产品化 + 近无彩 canvas）+ **D 分域**。
- 若要保留纸灯故事：**B 霜铜修订** 最小成本。
- 下一步需用户一句方向；实施再动 DESIGN/CSS。
