//! `vod_play_url` 两级嵌套树解析 — 按 `PlayUrlParserSpec` 分隔符 grammar 产 `Vec<serde_json::Value>`。
//!
//! 纯字符串处理,与 XML/JSON 引擎无关(ADR-0028 附则)。

/// `vod_play_url` 两级嵌套树解析的分隔符 grammar。
///
/// 定义 Maccms 视频源 `vod_play_url` / `vod_play_from` 字段的解析规则。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayUrlParserSpec {
    /// 线路间分隔符（默认 `###`）。
    pub line_sep: String,
    /// 集间分隔符（默认 `#`）。
    pub episode_sep: String,
    /// 集名-URL 分隔符（默认 `$`）。
    pub name_url_sep: String,
    /// 线路名分隔符（默认 `,`）。
    pub play_from_sep: String,
}

/// 按 `PlayUrlParserSpec` 分隔符 grammar 解析 `vod_play_url` + `vod_play_from` 产播放线路。
///
/// `vod_play_url` 格式: `<线路1集1名>$<url1>#<线路1集2名>$<url2>#...###<线路2...>`
/// `vod_play_from` 格式: `<线路1名>,<线路2名>,...`(按序与线路绑定)
///
/// # Errors
///
/// 返回 `NoMatch` 当线路数不一致或集内缺少名-URL 分隔符。
pub fn parse_play_lines(
    vod_play_url: &str,
    vod_play_from: &str,
    spec: &PlayUrlParserSpec,
) -> Result<Vec<serde_json::Value>, crate::error::ExtractError> {
    let line_names: Vec<String> = vod_play_from
        .split(&spec.play_from_sep)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let lines: Vec<&str> = vod_play_url
        .split(&spec.line_sep)
        .filter(|s| !s.is_empty())
        .collect();

    // vod_play_from 线路名数应与 vod_play_url 线路数一致(按序绑定)
    if line_names.len() != lines.len() {
        return Err(crate::error::ExtractError::NoMatch(format!(
            "play_url_parser 分隔符不匹配: vod_play_from 线路数 {} != vod_play_url 线路数 {}",
            line_names.len(),
            lines.len()
        )));
    }

    let mut play_lines = Vec::with_capacity(lines.len());
    for (line_url_str, name) in lines.iter().zip(line_names.iter()) {
        let episodes = parse_episodes(line_url_str, spec)?;
        if episodes.is_empty() {
            continue;
        }
        play_lines.push(serde_json::json!({ "name": name, "episodes": episodes }));
    }
    Ok(play_lines)
}

/// 解析单线路的分集列表(集间以 `episode_sep` 分隔,每集 `title$url`)。
fn parse_episodes(
    line_url_str: &str,
    spec: &PlayUrlParserSpec,
) -> Result<Vec<serde_json::Value>, crate::error::ExtractError> {
    let mut episodes = Vec::new();
    for ep_str in line_url_str.split(&spec.episode_sep) {
        let ep_str = ep_str.trim();
        if ep_str.is_empty() {
            continue;
        }
        let mut parts = ep_str.splitn(2, &spec.name_url_sep);
        let title = parts.next().unwrap_or("").trim().to_string();
        let url = parts.next().unwrap_or("").trim().to_string();
        if url.is_empty() {
            return Err(crate::error::ExtractError::NoMatch(format!(
                "play_url_parser 集 '{ep_str}' 缺少 name_url_sep '{}'",
                spec.name_url_sep
            )));
        }
        episodes.push(serde_json::json!({ "title": title, "url": url }));
    }
    Ok(episodes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> PlayUrlParserSpec {
        PlayUrlParserSpec {
            line_sep: "###".to_string(),
            episode_sep: "#".to_string(),
            name_url_sep: "$".to_string(),
            play_from_sep: ",".to_string(),
        }
    }

    #[test]
    fn parse_multi_line_multi_episode() {
        let spec = defaults();
        let url = "第1集$http://x/1.m3u8#第2集$http://x/2.m3u8###第1集$http://y/1.m3u8";
        let from = "hnyun,hnm3u8";
        let lines = parse_play_lines(url, from, &spec).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0]["name"], "hnyun");
        assert_eq!(lines[0]["episodes"].as_array().unwrap().len(), 2);
        assert_eq!(lines[0]["episodes"][0]["title"], "第1集");
        assert_eq!(lines[0]["episodes"][0]["url"], "http://x/1.m3u8");
        assert_eq!(lines[0]["episodes"][1]["title"], "第2集");
        assert_eq!(lines[1]["name"], "hnm3u8");
        assert_eq!(lines[1]["episodes"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn parse_single_line_single_episode() {
        let spec = defaults();
        let url = "正片$http://localhost/v_show/id_XMTM0.html";
        let from = "youku";
        let lines = parse_play_lines(url, from, &spec).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["name"], "youku");
        assert_eq!(lines[0]["episodes"].as_array().unwrap().len(), 1);
        assert_eq!(lines[0]["episodes"][0]["title"], "正片");
    }

    #[test]
    fn parse_line_count_mismatch_errors() {
        let spec = defaults();
        let url = "第1集$http://x/1.m3u8###第2集$http://x/2.m3u8";
        let from = "hnyun"; // 1 名 vs 2 线路
        assert!(parse_play_lines(url, from, &spec).is_err());
    }

    #[test]
    fn parse_missing_name_url_sep_errors() {
        let spec = defaults();
        let url = "第1集"; // 无 $ 分隔
        let from = "hnyun";
        assert!(parse_play_lines(url, from, &spec).is_err());
    }

    #[test]
    fn parse_empty_url_returns_empty() {
        let spec = defaults();
        let lines = parse_play_lines("", "", &spec).unwrap();
        assert!(lines.is_empty());
    }
}
