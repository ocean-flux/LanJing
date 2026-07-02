//! 媒体模型 — 规则执行产出的标准化媒体数据。

use serde::{Deserialize, Serialize};

/// 媒体类型枚举(ADR-0005 分层 enum)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Media {
    /// 图书媒体。
    Book(BookMedia),
    /// 视频媒体。
    Video(VideoMedia),
    /// 音频媒体(首刀 stub)。
    Audio(AudioMedia),
}

/// 图书媒体。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BookMedia {
    /// 书名。
    pub title: String,
    /// 作者(可选)。
    pub author: Option<String>,
    /// 封面图 URL。
    pub cover_url: Option<String>,
    /// 书籍简介。
    pub description: Option<String>,
    /// 类型:玄幻/都市等。
    pub kind: Option<String>,
    /// 最后一章标题。
    pub last_chapter: Option<String>,
    /// 详情页 URL。
    pub book_url: Option<String>,
    /// 章节列表(toc 端点产出)。
    pub chapters: Vec<BookChapter>,
}

/// 图书章节。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BookChapter {
    /// 章节标题。
    pub title: String,
    /// 章节页 URL。
    pub chapter_url: String,
    /// 正文(content 端点产出)。
    pub content: Option<String>,
}

/// 视频媒体(以播放线路为组织核心,ADR-0027)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoMedia {
    /// 视频标题。
    pub title: String,
    /// 封面图 URL。
    pub cover_url: Option<String>,
    /// 简介。
    pub description: Option<String>,
    /// 类型(国产剧/短剧等)。
    pub kind: Option<String>,
    /// 备注(更新状态等)。
    pub remarks: Option<String>,
    /// 站点内视频 ID(用于 Detail 端点请求)。
    pub vod_id: Option<String>,
    /// 播放线路列表(同一部剧可有多条播放源,每条自带完整分集)。
    pub play_lines: Vec<PlayLine>,
}

/// 播放线路(一条播放源带其完整分集列表,ADR-0027)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayLine {
    /// 线路名称(如 "hnyun"/"hnm3u8")。
    pub name: String,
    /// 分集列表。
    pub episodes: Vec<VideoEpisode>,
}

/// 视频分集(单集标题与流地址)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoEpisode {
    /// 分集标题。
    pub title: String,
    /// 流地址 URL。
    pub url: String,
}

/// 音频媒体(首刀 stub)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioMedia {
    /// 音频标题。
    pub title: String,
    /// 音频 URL。
    pub url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_media_with_multi_line_multi_episode() {
        let vm = VideoMedia {
            title: "爱情没有神话".to_string(),
            cover_url: Some("http://x/p.jpg".to_string()),
            description: None,
            kind: Some("国产剧".to_string()),
            remarks: Some("全集完结".to_string()),
            vod_id: Some("140789".to_string()),
            play_lines: vec![
                PlayLine {
                    name: "hnyun".to_string(),
                    episodes: vec![
                        VideoEpisode {
                            title: "第1集".to_string(),
                            url: "http://x/1.m3u8".to_string(),
                        },
                        VideoEpisode {
                            title: "第2集".to_string(),
                            url: "http://x/2.m3u8".to_string(),
                        },
                        VideoEpisode {
                            title: "第3集".to_string(),
                            url: "http://x/3.m3u8".to_string(),
                        },
                    ],
                },
                PlayLine {
                    name: "hnm3u8".to_string(),
                    episodes: vec![
                        VideoEpisode {
                            title: "第1集".to_string(),
                            url: "http://y/1.m3u8".to_string(),
                        },
                        VideoEpisode {
                            title: "第2集".to_string(),
                            url: "http://y/2.m3u8".to_string(),
                        },
                        VideoEpisode {
                            title: "第3集".to_string(),
                            url: "http://y/3.m3u8".to_string(),
                        },
                    ],
                },
            ],
        };
        assert_eq!(vm.play_lines.len(), 2);
        assert_eq!(vm.play_lines[0].name, "hnyun");
        assert_eq!(vm.play_lines[0].episodes.len(), 3);
        assert_eq!(vm.play_lines[1].episodes.len(), 3);
    }

    #[test]
    fn video_media_empty_play_lines() {
        let vm = VideoMedia {
            title: "test".to_string(),
            cover_url: None,
            description: None,
            kind: None,
            remarks: None,
            vod_id: None,
            play_lines: vec![],
        };
        assert!(vm.play_lines.is_empty());
    }
}
