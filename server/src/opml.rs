use anyhow::{Context, Result};

/// OPML 里解析出来的一条订阅
#[derive(Debug, Clone)]
pub struct OpmlEntry {
    pub title: String,
    pub url: String,
    pub category: String,
}

/// 解析 OPML：支持带目录分组（嵌套 outline）和平铺两种写法
pub fn parse(xml: &str) -> Result<Vec<OpmlEntry>> {
    let doc = roxmltree::Document::parse(xml).context("OPML 不是合法的 XML")?;
    let mut out = Vec::new();
    let mut stack: Vec<String> = Vec::new();

    visit(doc.root_element(), &mut stack, &mut out);
    Ok(out)
}

fn visit(node: roxmltree::Node, stack: &mut Vec<String>, out: &mut Vec<OpmlEntry>) {
    let is_outline = node.has_tag_name("outline");
    let url = node
        .attribute("xmlUrl")
        .or_else(|| node.attribute("xmlurl"));
    let text = node
        .attribute("text")
        .or_else(|| node.attribute("title"))
        .unwrap_or("")
        .to_string();

    if is_outline {
        match url {
            Some(u) => {
                let category = stack.join("/");
                let title = if text.trim().is_empty() {
                    u.to_string()
                } else {
                    text
                };
                let u = u.trim();
                if !u.is_empty() {
                    out.push(OpmlEntry {
                        title: title.trim().to_string(),
                        url: u.to_string(),
                        category,
                    });
                }
                return; // 订阅项不再往下钻
            }
            None => {
                // 目录节点
                if !text.trim().is_empty() {
                    stack.push(text.trim().to_string());
                    for child in node.children().filter(|c| c.is_element()) {
                        visit(child, stack, out);
                    }
                    stack.pop();
                }
                return;
            }
        }
    }

    for child in node.children().filter(|c| c.is_element()) {
        visit(child, stack, out);
    }
}

/// 生成 OPML 2.0
pub fn generate(items: &[OpmlEntry], doc_title: &str) -> String {
    let mut s = String::new();
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    s.push_str("<opml version=\"2.0\">\n");
    s.push_str("  <head>\n");
    s.push_str(&format!("    <title>{}</title>\n", escape(doc_title)));
    s.push_str(&format!(
        "    <dateCreated>{}</dateCreated>\n",
        chrono::Utc::now().format("%a, %d %b %Y %H:%M:%S +0000")
    ));
    s.push_str("  </head>\n");
    s.push_str("  <body>\n");

    // 按 category 分组输出，保持目录结构
    let mut groups: Vec<(&str, Vec<&OpmlEntry>)> = Vec::new();
    for it in items {
        let key = it.category.as_str();
        if let Some(g) = groups.iter_mut().find(|(k, _)| *k == key) {
            g.1.push(it);
        } else {
            groups.push((key, vec![it]));
        }
    }

    for (category, entries) in groups {
        if category.is_empty() {
            for e in entries {
                s.push_str(&format!(
                    "    <outline type=\"rss\" text=\"{}\" xmlUrl=\"{}\"/>\n",
                    escape(&e.title),
                    escape(&e.url)
                ));
            }
        } else {
            // 支持 a/b/c 这种多级目录
            let parts: Vec<&str> = category.split('/').filter(|p| !p.is_empty()).collect();
            let indent = |depth: usize| "    ".repeat(depth + 1);
            for (i, part) in parts.iter().enumerate() {
                s.push_str(&format!(
                    "{}<outline text=\"{}\">\n",
                    indent(i),
                    escape(part)
                ));
            }
            let base = parts.len();
            for e in entries {
                s.push_str(&format!(
                    "{}<outline type=\"rss\" text=\"{}\" xmlUrl=\"{}\"/>\n",
                    indent(base),
                    escape(&e.title),
                    escape(&e.url)
                ));
            }
            for i in (0..parts.len()).rev() {
                s.push_str(&format!("{}</outline>\n", indent(i)));
            }
        }
    }

    s.push_str("  </body>\n");
    s.push_str("</opml>\n");
    s
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
