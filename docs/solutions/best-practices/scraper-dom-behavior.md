---
title: scraper crate DOM 行为 — innerHTML vs outerHTML + 自动补全骨架
date: 2026-06-27
category: docs/solutions/best-practices/
module: node-extract
problem_type: best_practice
component: service_object
severity: medium
applies_when:
  - "使用 scraper crate 解析 HTML 片段并提取元素属性"
  - "从列表项元素提取 href/src 等属性"
  - "CSS 选择器在 scraper 中匹配到意外元素"
tags:
  - scraper
  - html
  - dom
  - css-selector
  - inner-html
  - outer-html
  - extraction
---

# scraper crate DOM 行为 — innerHTML vs outerHTML + 自动补全骨架

## Context

LanJing 的 ExtractNodeProcessor 用 `scraper` crate 解析 HTML 并提取内容。在 Toc 端点调试中,`chapterUrl` 的 `href` 属性始终提取失败 — 根因是 scraper 的两个非直觉行为:`inner_html()` vs `html()` 的差异,以及自动补全 HTML 骨架对 CSS 选择器的影响。

## Guidance

### 1. `inner_html()` vs `html()` — outerHTML 保留元素自身标签

```rust
use scraper::ElementRef;

let element: ElementRef = doc.select(&selector).next().unwrap();

// inner_html(): 返回元素内部的 HTML,不含元素自身标签
// <a href="/book/123">章节名</a> → "章节名"
let inner = element.inner_html();

// html(): 返回 outerHTML,含元素自身标签和属性
// <a href="/book/123">章节名</a> → "<a href=\"/book/123\">章节名</a>"
let outer = element.html();
```

**提取元素属性(如 `href`/`src`)时必须用 `html()`(outerHTML)**,因为属性在元素自身的标签上,`inner_html()` 丢弃了它。

### 2. scraper 自动补全 HTML 骨架

scraper 解析 HTML 片段时会自动补全完整的 HTML 结构:

```rust
let fragment = "<a href=\"/book/123\">章节名</a>";
let doc = Html::parse_document(fragment);
// scraper 内部构建: <html><head></head><body><a href="/book/123">章节名</a></body></html>
```

**`*` 选择器匹配到 `<html>` 根元素,不是用户提供的片段**。要选中片段本身,用 `body > *`:

```rust
// 错误:* 匹配到 <html>(scraper 自动补全的根)
let selector = Selector::parse("*").unwrap();
let element = doc.select(&selector).next();  // → <html> 元素

// 正确:body > * 匹配 <body> 下第一个子元素(用户提供的片段)
let selector = Selector::parse("body > *").unwrap();
let element = doc.select(&selector).next();  // → <a> 元素
```

### 3. 元素级提取 — 避免 serialize→reparse round-trip

从列表项提取多个字段时,不要先 `el.html()` 序列化成字符串再 `Html::parse_document()` 重新解析。直接在 `ElementRef` 上用子选择器:

```rust
// 反模式:serialize → reparse(每个 item 一次无谓的 HTML parse)
let items: Vec<String> = doc.select(&list_sel).map(|el| el.html()).collect();
for item_html in items {
    let item_doc = Html::parse_document(&item_html);  // 重新解析!
    let title = extract_from_doc(&item_doc, "h2");
}

// 正确模式:直接在 ElementRef 上用子选择器
let elements: Vec<ElementRef> = doc.select(&list_sel).collect();
for el in elements {
    let title = el.select(&title_sel).next().and_then(|e| e.text().next());
    let href = el.value().attr("href");  // 直接访问属性
}
```

## Why This Matters

- **属性提取失败是静默的**:`href` 返回 `None` 不报错,只是章节 URL 为空,难以诊断
- **`*` 选择器匹配 `<html>` 是非直觉的**:用户以为 `*` 匹配"任何元素",实际匹配到自动补全的根
- **serialize→reparse 在列表场景开销显著**:Toc 端点可能有几百章,每章一次 HTML parse 是纯浪费
- **scraper 的行为与浏览器 DOM 不同**:浏览器中 `element.innerHTML` 和 `element.outerHTML` 的语义一致,但 scraper 的 `inner_html()` 和 `html()` 命名不对应

## When to Apply

- 使用 scraper crate 解析 HTML 并提取元素属性(href/src/data-*)
- 从 HTML 列表中提取多个字段的每个列表项
- CSS 选择器在 scraper 中匹配到意外元素时
- 评估 HTML 提取性能(列表场景的 serialize→reparse 开销)

## Examples

### Toc 章节提取完整模式

```rust
use scraper::{Html, Selector, ElementRef};

fn extract_toc(doc: &Html, list_selector: &str) -> Vec<(String, String)> {
    let list_sel = Selector::parse(list_selector).unwrap();
    let name_sel = Selector::parse("a").unwrap();  // 章节名在 <a> 的文本中

    doc.select(&list_sel)
        .filter_map(|el: ElementRef| {
            let name = el.select(&name_sel)
                .next()?
                .text()
                .collect::<String>();
            let url = el.select(&name_sel)
                .next()?
                .value()
                .attr("href")?;  // 直接从 ElementRef 访问属性
            Some((name, url.to_string()))
        })
        .collect()
}
```

## Related Issues

- `docs/solutions/integration-issues/legado-engine-full-chain-integration.md` — Bug 6 innerHTML vs outerHTML 修复
- scraper crate 文档:https://docs.rs/scraper
