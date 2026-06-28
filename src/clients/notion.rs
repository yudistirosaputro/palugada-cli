//! Notion doc source. `base_url` defaults to `https://api.notion.com`; auth
//! reuses the doc-source token (`wiki_token`) as a Bearer token with the
//! required `Notion-Version` header. `get_page` fetches the page (for its
//! title) and walks its block tree (paginated + recursive) into a Markdown
//! body. The block→Markdown rendering lives in pure helpers (`render_nodes`)
//! so it is unit-testable; only `fetch_children` touches the network, and it
//! degrades (returns whatever it gathered) on a transient child/page error
//! rather than failing the whole fetch.

use super::{DocSource, WikiPage};
use crate::http::Http;
use serde::Deserialize;

const NOTION_VERSION: &str = "2022-06-28";
/// Guard against a pathological/cyclic block tree (Notion nesting is shallow).
const MAX_DEPTH: usize = 16;
/// Belt-and-suspenders cap on `/children` pages per parent (100 blocks each).
const MAX_PAGES: usize = 10_000;

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

    /// Walk a block's children into a node tree: paginate `/children` following
    /// `has_more`/`next_cursor`, and recurse into any block that `has_children`
    /// (tables hold their rows this way; nested lists hold their sub-items).
    /// Transient errors (HTTP, parse, non-advancing cursor, depth) end the walk
    /// for that subtree and return what was gathered — a degraded body still
    /// loads, matching the old single-call renderer's tolerance.
    fn fetch_children(&self, parent_id: &str, depth: usize) -> Vec<BlockNode> {
        let mut nodes: Vec<BlockNode> = Vec::new();
        if depth >= MAX_DEPTH {
            return nodes;
        }
        let mut cursor: Option<String> = None;
        let mut pages = 0usize;
        loop {
            let mut url = format!(
                "{}/v1/blocks/{}/children?page_size=100",
                self.base_url,
                crate::http::encode_segment(parent_id)
            );
            if let Some(c) = &cursor {
                url.push_str(&format!("&start_cursor={}", crate::http::encode_segment(c)));
            }
            let json = match self.http.get_text(&url, &self.headers()) {
                Ok(j) => j,
                Err(_) => break,
            };
            let resp: BlocksResp = match serde_json::from_str(&json) {
                Ok(r) => r,
                Err(_) => break,
            };
            for b in resp.results {
                let children = if b.has_children && !b.id.is_empty() {
                    self.fetch_children(&b.id, depth + 1)
                } else {
                    Vec::new()
                };
                nodes.push(BlockNode { block: b, children });
            }
            pages += 1;
            match (resp.has_more, resp.next_cursor) {
                (true, Some(next)) if Some(&next) != cursor.as_ref() && pages < MAX_PAGES => {
                    cursor = Some(next);
                }
                _ => break,
            }
        }
        nodes
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
    #[serde(default)]
    has_more: bool,
    #[serde(default)]
    next_cursor: Option<String>,
}

#[derive(Deserialize)]
struct Block {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    id: String,
    #[serde(default)]
    has_children: bool,
    paragraph: Option<TextHolder>,
    heading_1: Option<TextHolder>,
    heading_2: Option<TextHolder>,
    heading_3: Option<TextHolder>,
    bulleted_list_item: Option<TextHolder>,
    numbered_list_item: Option<TextHolder>,
    to_do: Option<ToDoHolder>,
    quote: Option<TextHolder>,
    code: Option<CodeHolder>,
    table_row: Option<TableRow>,
}

#[derive(Deserialize)]
struct TextHolder {
    #[serde(default)]
    rich_text: Vec<RichText>,
}

#[derive(Deserialize)]
struct ToDoHolder {
    #[serde(default)]
    rich_text: Vec<RichText>,
    #[serde(default)]
    checked: bool,
}

#[derive(Deserialize)]
struct CodeHolder {
    #[serde(default)]
    rich_text: Vec<RichText>,
    #[serde(default)]
    language: String,
}

#[derive(Deserialize)]
struct TableRow {
    #[serde(default)]
    cells: Vec<Vec<RichText>>,
}

/// A block plus its (already-fetched) child blocks.
struct BlockNode {
    block: Block,
    children: Vec<BlockNode>,
}

fn text_of(h: &Option<TextHolder>) -> String {
    h.as_ref().map(|x| join_text(&x.rich_text)).unwrap_or_default()
}

/// A code fence long enough to wrap `body` even if it contains ``` runs.
fn code_fence(body: &str) -> String {
    let max = body
        .lines()
        .map(|l| l.trim_start().chars().take_while(|&c| c == '`').count())
        .max()
        .unwrap_or(0);
    "`".repeat(max.max(2) + 1)
}

/// One block's own Markdown, or `None` for an empty/unknown block (its children
/// are recursed separately by `render_nodes`). `numbered_list_item`, `table`,
/// and `table_row` are handled directly in `render_nodes` (ordinal/row context).
fn render_block_line(b: &Block, depth: usize) -> Option<String> {
    let indent = "  ".repeat(depth);
    let non_empty = |t: String| -> Option<String> {
        let t = t.trim().to_string();
        (!t.is_empty()).then_some(t)
    };
    match b.kind.as_str() {
        "heading_1" => non_empty(text_of(&b.heading_1)).map(|t| format!("# {t}")),
        "heading_2" => non_empty(text_of(&b.heading_2)).map(|t| format!("## {t}")),
        "heading_3" => non_empty(text_of(&b.heading_3)).map(|t| format!("### {t}")),
        "paragraph" => non_empty(text_of(&b.paragraph)),
        "quote" => non_empty(text_of(&b.quote)).map(|t| format!("> {t}")),
        "bulleted_list_item" => {
            non_empty(text_of(&b.bulleted_list_item)).map(|t| format!("{indent}- {t}"))
        }
        "to_do" => {
            let h = b.to_do.as_ref();
            let t = h.map(|x| join_text(&x.rich_text)).unwrap_or_default();
            non_empty(t).map(|t| {
                let checked = h.map(|x| x.checked).unwrap_or(false);
                format!("{indent}- [{}] {t}", if checked { "x" } else { " " })
            })
        }
        "code" => {
            let h = b.code.as_ref();
            let lang = h.map(|x| x.language.as_str()).unwrap_or("");
            let body = h.map(|x| join_text(&x.rich_text)).unwrap_or_default();
            let fence = code_fence(&body);
            Some(format!("{fence}{lang}\n{body}\n{fence}"))
        }
        _ => None,
    }
}

/// Escape a table cell so it can't break the pipe table.
fn table_cell(rt: &[RichText]) -> String {
    join_text(rt).replace('\n', " ").replace('|', "\\|")
}

/// Render a `table` block (rows are its children) as a Markdown pipe table,
/// indented for `depth` and surrounded by blank lines by the caller.
fn render_table(node: &BlockNode, out: &mut Vec<String>, depth: usize) {
    let indent = "  ".repeat(depth);
    let rows: Vec<Vec<String>> = node
        .children
        .iter()
        .filter_map(|c| c.block.table_row.as_ref())
        .map(|tr| tr.cells.iter().map(|c| table_cell(c)).collect())
        .collect();
    if rows.is_empty() {
        return;
    }
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0).max(1);
    let line = |r: &Vec<String>| {
        let mut cells = r.clone();
        cells.resize(cols, String::new());
        format!("{indent}| {} |", cells.join(" | "))
    };
    out.push(line(&rows[0]));
    out.push(format!("{indent}| {} |", vec!["---"; cols].join(" | ")));
    for r in &rows[1..] {
        out.push(line(r));
    }
}

/// Push a blank separator line, unless `out` is empty or already ends in one.
fn push_blank(out: &mut Vec<String>) {
    if out.last().map(|s| !s.is_empty()).unwrap_or(false) {
        out.push(String::new());
    }
}

/// Render a node list to Markdown lines. Numbered ordinals are tracked per
/// level and only reset when a *rendered* non-numbered sibling breaks the run
/// (a no-op block like an image keeps the sequence going). Code/table blocks
/// get blank-line separation so GFM recognizes them as their own block.
fn render_nodes(nodes: &[BlockNode], out: &mut Vec<String>, depth: usize) {
    let mut num = 0usize;
    for node in nodes {
        let b = &node.block;
        match b.kind.as_str() {
            "table_row" => continue,
            "table" => {
                push_blank(out);
                render_table(node, out, depth);
                push_blank(out);
                num = 0;
                continue;
            }
            "numbered_list_item" => {
                if let Some(t) = {
                    let t = text_of(&b.numbered_list_item);
                    let t = t.trim().to_string();
                    (!t.is_empty()).then_some(t)
                } {
                    num += 1;
                    out.push(format!("{}{num}. {t}", "  ".repeat(depth)));
                }
                if !node.children.is_empty() {
                    render_nodes(&node.children, out, depth + 1);
                }
                continue;
            }
            _ => {}
        }
        if let Some(line) = render_block_line(b, depth) {
            if b.kind == "code" {
                push_blank(out);
                out.push(line);
                push_blank(out);
            } else {
                out.push(line);
            }
            num = 0;
        }
        if !node.children.is_empty() {
            render_nodes(&node.children, out, depth + 1);
        }
    }
}

/// Join rendered lines into the final body: collapse runs of blank lines and
/// trim leading/trailing blanks.
fn to_markdown(lines: Vec<String>) -> String {
    let mut out: Vec<String> = Vec::new();
    for l in lines {
        if l.is_empty() && out.last().map(|s| s.is_empty()).unwrap_or(true) {
            continue;
        }
        out.push(l);
    }
    while out.last().map(|s| s.is_empty()).unwrap_or(false) {
        out.pop();
    }
    out.join("\n")
}

/// Render a single `/children` JSON page (no pagination, no child fetch) to
/// Markdown — a test helper exercising the parse→render path. The real walk
/// goes through `Notion::fetch_children` + `render_nodes`.
#[cfg(test)]
fn render_blocks(json: &str) -> String {
    let resp: BlocksResp = match serde_json::from_str(json) {
        Ok(r) => r,
        Err(_) => return String::new(),
    };
    let nodes: Vec<BlockNode> =
        resp.results.into_iter().map(|block| BlockNode { block, children: Vec::new() }).collect();
    let mut lines = Vec::new();
    render_nodes(&nodes, &mut lines, 0);
    to_markdown(lines)
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
        let page_json =
            self.http.get_text(&format!("{}/v1/pages/{}", self.base_url, seg), &self.headers())?;
        let nodes = self.fetch_children(id, 0);
        let mut lines = Vec::new();
        render_nodes(&nodes, &mut lines, 0);
        Ok(WikiPage {
            id: id.to_string(),
            title: page_title(&page_json),
            body_html: to_markdown(lines),
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
    use super::{render_blocks, render_nodes, to_markdown, page_title, Block, BlockNode, BlocksResp};

    fn blk(json: &str) -> Block {
        serde_json::from_str(json).expect("block json")
    }
    fn node(json: &str, children: Vec<BlockNode>) -> BlockNode {
        BlockNode { block: blk(json), children }
    }
    fn render(nodes: &[BlockNode]) -> String {
        let mut out = Vec::new();
        render_nodes(nodes, &mut out, 0);
        to_markdown(out)
    }

    #[test]
    fn page_title_reads_the_title_property() {
        let json = r#"{"properties":{"Name":{"type":"title","title":[{"plain_text":"My "},{"plain_text":"Page"}]},"Tags":{"type":"multi_select"}}}"#;
        assert_eq!(page_title(json), "My Page");
        assert_eq!(page_title(r#"{"properties":{}}"#), "");
    }

    #[test]
    fn render_blocks_emits_markdown_structure_and_skips_unknown() {
        let json = r#"{"results":[
            {"type":"heading_1","heading_1":{"rich_text":[{"plain_text":"Title"}]}},
            {"type":"paragraph","paragraph":{"rich_text":[{"plain_text":"Hello "},{"plain_text":"world"}]}},
            {"type":"image","image":{}},
            {"type":"bulleted_list_item","bulleted_list_item":{"rich_text":[{"plain_text":"point"}]}}
        ]}"#;
        assert_eq!(render_blocks(json), "# Title\nHello world\n- point");
    }

    #[test]
    fn blocks_resp_parses_pagination_fields() {
        let resp: BlocksResp =
            serde_json::from_str(r#"{"results":[],"has_more":true,"next_cursor":"cur-123"}"#).unwrap();
        assert!(resp.has_more);
        assert_eq!(resp.next_cursor.as_deref(), Some("cur-123"));
        let last: BlocksResp = serde_json::from_str(r#"{"results":[]}"#).unwrap();
        assert!(!last.has_more);
        assert_eq!(last.next_cursor, None);
    }

    #[test]
    fn code_block_is_fenced_with_language() {
        let n = node(
            r#"{"type":"code","code":{"language":"rust","rich_text":[{"plain_text":"let x = 1;"}]}}"#,
            vec![],
        );
        assert_eq!(render(&[n]), "```rust\nlet x = 1;\n```");
    }

    #[test]
    fn code_block_with_internal_backticks_uses_a_longer_fence() {
        // Body contains a ```` ``` ```` line; the fence must be longer so it doesn't break out.
        let n = node(
            r#"{"type":"code","code":{"language":"","rich_text":[{"plain_text":"```\nnested"}]}}"#,
            vec![],
        );
        assert_eq!(render(&[n]), "````\n```\nnested\n````");
    }

    #[test]
    fn table_renders_as_markdown_pipe_table() {
        let table = node(
            r#"{"type":"table","has_children":true,"table":{"has_column_header":true}}"#,
            vec![
                node(r#"{"type":"table_row","table_row":{"cells":[[{"plain_text":"A"}],[{"plain_text":"B"}]]}}"#, vec![]),
                node(r#"{"type":"table_row","table_row":{"cells":[[{"plain_text":"1"}],[{"plain_text":"2"}]]}}"#, vec![]),
            ],
        );
        assert_eq!(render(&[table]), "| A | B |\n| --- | --- |\n| 1 | 2 |");
    }

    #[test]
    fn table_after_paragraph_gets_a_blank_line_separator() {
        let nodes = vec![
            node(r#"{"type":"paragraph","paragraph":{"rich_text":[{"plain_text":"intro"}]}}"#, vec![]),
            node(
                r#"{"type":"table","has_children":true}"#,
                vec![node(r#"{"type":"table_row","table_row":{"cells":[[{"plain_text":"A"}]]}}"#, vec![])],
            ),
        ];
        assert_eq!(render(&nodes), "intro\n\n| A |\n| --- |");
    }

    #[test]
    fn nested_list_children_are_recursed_and_indented() {
        let parent = node(
            r#"{"type":"bulleted_list_item","has_children":true,"bulleted_list_item":{"rich_text":[{"plain_text":"parent"}]}}"#,
            vec![
                node(r#"{"type":"numbered_list_item","numbered_list_item":{"rich_text":[{"plain_text":"first"}]}}"#, vec![]),
                node(r#"{"type":"numbered_list_item","numbered_list_item":{"rich_text":[{"plain_text":"second"}]}}"#, vec![]),
            ],
        );
        assert_eq!(render(&[parent]), "- parent\n  1. first\n  2. second");
    }

    #[test]
    fn numbered_run_continues_across_a_non_emitting_block_but_skips_empty_items() {
        let nodes = vec![
            node(r#"{"type":"numbered_list_item","numbered_list_item":{"rich_text":[{"plain_text":"a"}]}}"#, vec![]),
            node(r#"{"type":"image","image":{}}"#, vec![]), // emits nothing → must not reset the count
            node(r#"{"type":"numbered_list_item","numbered_list_item":{"rich_text":[{"plain_text":"  "}]}}"#, vec![]), // empty → skipped, no number consumed
            node(r#"{"type":"numbered_list_item","numbered_list_item":{"rich_text":[{"plain_text":"b"}]}}"#, vec![]),
        ];
        assert_eq!(render(&nodes), "1. a\n2. b");
    }

    #[test]
    fn todo_state_and_unknown_container_children_preserved() {
        let done = node(r#"{"type":"to_do","to_do":{"checked":true,"rich_text":[{"plain_text":"done"}]}}"#, vec![]);
        assert_eq!(render(&[done]), "- [x] done");
        // An unknown container (e.g. column_list) contributes no line of its own
        // but its children are still rendered.
        let col = node(
            r#"{"type":"column_list","has_children":true}"#,
            vec![node(r#"{"type":"paragraph","paragraph":{"rich_text":[{"plain_text":"inside"}]}}"#, vec![])],
        );
        assert_eq!(render(&[col]), "inside");
    }

    #[test]
    fn empty_and_whitespace_only_blocks_are_dropped() {
        let nodes = vec![
            node(r#"{"type":"bulleted_list_item","bulleted_list_item":{"rich_text":[]}}"#, vec![]),
            node(r#"{"type":"heading_2","heading_2":{"rich_text":[{"plain_text":"   "}]}}"#, vec![]),
            node(r#"{"type":"paragraph","paragraph":{"rich_text":[{"plain_text":"kept"}]}}"#, vec![]),
        ];
        assert_eq!(render(&nodes), "kept");
        assert_eq!(to_markdown(vec![String::new(), "x".into(), String::new(), String::new(), "y".into(), String::new()]), "x\n\ny");
    }
}
