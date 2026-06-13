//! Notion doc source. `base_url` defaults to `https://api.notion.com`; auth
//! reuses the doc-source token (`wiki_token`) as a Bearer token with the
//! required `Notion-Version` header. `get_page` fetches the page (for its title)
//! and its child blocks (for plain-text body); response parsing lives in the
//! pure `page_title` / `render_blocks` helpers so it is unit-testable.

use super::{DocSource, WikiPage};
use crate::http::Http;
use serde::Deserialize;

const NOTION_VERSION: &str = "2022-06-28";

pub struct Notion {
    base_url: String,
    token: String,
    http: Http,
}

impl Notion {
    pub fn new(base_url: &str, token: &str, insecure: bool) -> Self {
        let base = if base_url.is_empty() {
            "https://api.notion.com".to_string()
        } else {
            base_url.trim_end_matches('/').to_string()
        };
        Notion { base_url: base, token: token.to_string(), http: Http::new(insecure) }
    }

    fn headers(&self) -> Vec<(&str, String)> {
        vec![
            ("Authorization", format!("Bearer {}", self.token)),
            ("Notion-Version", NOTION_VERSION.to_string()),
        ]
    }
}

#[derive(Deserialize)]
struct RichText {
    #[serde(default)]
    plain_text: String,
}

fn join_text(rt: &[RichText]) -> String {
    rt.iter().map(|t| t.plain_text.as_str()).collect::<String>()
}

// ── page title ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct PageResp {
    #[serde(default)]
    properties: std::collections::BTreeMap<String, Prop>,
}

#[derive(Deserialize)]
struct Prop {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    title: Vec<RichText>,
}

/// The page's title = the text of the single property whose type is "title".
fn page_title(json: &str) -> String {
    let page: PageResp = match serde_json::from_str(json) {
        Ok(p) => p,
        Err(_) => return String::new(),
    };
    page.properties
        .values()
        .find(|p| p.kind == "title")
        .map(|p| join_text(&p.title))
        .unwrap_or_default()
}

// ── block body ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct BlocksResp {
    #[serde(default)]
    results: Vec<Block>,
}

#[derive(Deserialize)]
struct Block {
    #[serde(rename = "type", default)]
    kind: String,
    paragraph: Option<TextHolder>,
    heading_1: Option<TextHolder>,
    heading_2: Option<TextHolder>,
    heading_3: Option<TextHolder>,
    bulleted_list_item: Option<TextHolder>,
    numbered_list_item: Option<TextHolder>,
    to_do: Option<TextHolder>,
    quote: Option<TextHolder>,
}

#[derive(Deserialize)]
struct TextHolder {
    #[serde(default)]
    rich_text: Vec<RichText>,
}

impl Block {
    fn holder(&self) -> Option<&TextHolder> {
        match self.kind.as_str() {
            "paragraph" => self.paragraph.as_ref(),
            "heading_1" => self.heading_1.as_ref(),
            "heading_2" => self.heading_2.as_ref(),
            "heading_3" => self.heading_3.as_ref(),
            "bulleted_list_item" => self.bulleted_list_item.as_ref(),
            "numbered_list_item" => self.numbered_list_item.as_ref(),
            "to_do" => self.to_do.as_ref(),
            "quote" => self.quote.as_ref(),
            _ => None,
        }
    }
}

/// Concatenate the plain text of the common text blocks, one line per block.
/// Unknown block types are skipped.
fn render_blocks(json: &str) -> String {
    let resp: BlocksResp = match serde_json::from_str(json) {
        Ok(r) => r,
        Err(_) => return String::new(),
    };
    resp.results
        .iter()
        .filter_map(|b| b.holder().map(|h| join_text(&h.rich_text)))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Deserialize)]
struct MeResp {
    name: Option<String>,
}

impl DocSource for Notion {
    fn get_page(&self, id: &str) -> Result<WikiPage, String> {
        if self.token.is_empty() {
            return Err("wiki_token is empty in the auth profile".into());
        }
        let seg = crate::http::encode_segment(id);
        let page_json = self.http.get_text(&format!("{}/v1/pages/{}", self.base_url, seg), &self.headers())?;
        let blocks_json = self
            .http
            .get_text(&format!("{}/v1/blocks/{}/children?page_size=100", self.base_url, seg), &self.headers())?;
        Ok(WikiPage {
            id: id.to_string(),
            title: page_title(&page_json),
            body_html: render_blocks(&blocks_json),
        })
    }

    fn verify(&self) -> Result<String, String> {
        if self.token.is_empty() {
            return Err("wiki_token is empty in the auth profile".into());
        }
        let me: MeResp = self.http.get_json(&format!("{}/v1/users/me", self.base_url), &self.headers())?;
        Ok(format!("Notion OK — authenticated as {}", me.name.unwrap_or_else(|| "?".to_string())))
    }
}

#[cfg(test)]
mod tests {
    use super::{page_title, render_blocks};

    #[test]
    fn page_title_reads_the_title_property() {
        let json = r#"{"properties":{"Name":{"type":"title","title":[{"plain_text":"My "},{"plain_text":"Page"}]},"Tags":{"type":"multi_select"}}}"#;
        assert_eq!(page_title(json), "My Page");
        assert_eq!(page_title(r#"{"properties":{}}"#), "");
    }

    #[test]
    fn render_blocks_joins_text_and_skips_unknown() {
        let json = r#"{"results":[
            {"type":"heading_1","heading_1":{"rich_text":[{"plain_text":"Title"}]}},
            {"type":"paragraph","paragraph":{"rich_text":[{"plain_text":"Hello "},{"plain_text":"world"}]}},
            {"type":"image","image":{}},
            {"type":"bulleted_list_item","bulleted_list_item":{"rich_text":[{"plain_text":"point"}]}}
        ]}"#;
        assert_eq!(render_blocks(json), "Title\nHello world\npoint");
    }
}
