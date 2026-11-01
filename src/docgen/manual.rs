/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */

//! The user manual, built from its Markdown sources by the same engine that
//! draws every PDF pgopr produces.
//!
//! # The manual's Markdown
//!
//! `doc/manual/en` holds one chapter per file, named `??-*.md` and built in
//! that order. The first file may open with YAML front matter (`title`,
//! `subtitle`) for the cover; a `\newpage` paragraph forces a page break;
//! `[label](#anchor)` links jump to their heading in the PDF; and the
//! `[id]: url` definitions may live in a file of their own.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use markdown::mdast::{Node, Table};

use super::{
    BRAND_COLOR, Block, CODE_SIZE, CONTENT_BOTTOM_MM, CONTENT_TOP_MM, MARGIN_MM, PAGE_HEIGHT_MM,
    PAGE_WIDTH_MM, PT_TO_MM, Pdf, Span, TEXT_COLOR, USABLE_WIDTH_MM, WHITE,
    collect_link_definitions, inline_spans_of, parse_markdown, render_block_nodes,
    resolve_reference_links,
};

/// The manual's band text, and how deep its table of contents goes.
const MANUAL_HEADER: &str = "pgopr";
const TOC_DEPTH: u8 = 2;
/// The cover's wordmark and tagline sizes.
const COVER_TITLE_SIZE: f32 = 64.0;
const COVER_SUBTITLE_SIZE: f32 = 20.0;
/// A table's cell padding, and the rule under its header row.
const CELL_PAD_MM: f32 = 2.0;
const TABLE_RULE: f32 = 0.4;
/// The footer band, and the site the whole line links to.
const FOOTER: &str = "2026 pgopr.github.io";
const FOOTER_URL: &str = "https://pgopr.github.io/";

/// The `??-*.md` sources of a document, in name order — one chapter each.
fn source_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut sources: Vec<PathBuf> = fs::read_dir(dir)
        .with_context(|| format!("failed to read {}", dir.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.extension().is_some_and(|ext| ext == "md")
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.len() > 3 && name.as_bytes()[2] == b'-')
        })
        .collect();
    sources.sort();
    if sources.is_empty() {
        bail!("no sources found in {} matching ??-*.md", dir.display());
    }
    Ok(sources)
}

/// Build `source_dir`'s Markdown into the manual at `output`: a brand cover, a
/// table of contents, then one chapter per source file. Returns the path
/// written.
pub fn build_manual(source_dir: &Path, output: &Path) -> Result<PathBuf> {
    let chapters = read_chapters(source_dir)?;
    let (title, subtitle) = front_matter(source_dir)?;

    // Two passes: the first learns which page each contents entry lands on,
    // knowing only that every chapter starts a fresh page, so the numbers hold
    // once the cover and the contents are pushed in front of them.
    let mut probe = Pdf::with_footer(MANUAL_HEADER, FOOTER, (FOOTER, FOOTER_URL))?;
    let outline = draw_chapters(&mut probe, &chapters)?;
    let toc_pages = toc_page_count(outline.entries.len());
    let offset = 1 + toc_pages;

    let mut pdf = Pdf::with_footer(MANUAL_HEADER, FOOTER, (FOOTER, FOOTER_URL))?;
    // The pages the probe saw, shifted past the cover and the contents: with
    // these, a `](#anchor)` link jumps to its heading instead of opening a
    // URL no viewer can follow.
    pdf.set_anchors(
        outline
            .anchors
            .iter()
            .map(|(slug, page)| (slug.clone(), page + offset))
            .collect(),
    );
    draw_cover(&mut pdf, &title, &subtitle);
    pdf.new_page();
    draw_contents(&mut pdf, &outline.entries, offset);
    while pdf.current_page() < offset {
        pdf.new_page();
    }
    pdf.new_page();
    draw_chapters(&mut pdf, &chapters)?;

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    pdf.save(output)?;
    Ok(output.to_path_buf())
}

use anyhow::Context as _;

/// One chapter: the pieces of one source file, in order.
struct Chapter {
    pieces: Vec<Piece>,
}

/// What a chapter is made of. Prose, lists and code go through the engine's
/// own Markdown rendering; tables and images are laid out here, because a
/// reference manual's tables have to be real columns rather than the
/// monospaced approximation.
enum Piece {
    Blocks(Vec<Block>),
    /// A heading: its depth, its numbered text, the anchor a `[label](#slug)`
    /// link elsewhere in the manual points at, and the blocks that draw it.
    Heading {
        depth: u8,
        title: String,
        slug: String,
        blocks: Vec<Block>,
    },
    Table(Vec<Vec<Vec<Span>>>),
    Image(PathBuf),
    PageBreak,
}

fn read_chapters(dir: &Path) -> Result<Vec<Chapter>> {
    let sources = source_files(dir)?;
    let mut texts = Vec::with_capacity(sources.len());
    for path in &sources {
        texts.push(
            fs::read_to_string(path)
                .with_context(|| format!("failed to read {}", path.display()))?,
        );
    }
    let definitions = link_definitions(&texts);

    let mut numbers = [0usize; 6];
    let chapters = sources
        .iter()
        .zip(&texts)
        .map(|(path, markdown)| read_chapter(path, markdown, &definitions, &mut numbers))
        .collect::<Result<Vec<_>>>()?;
    // A file that holds nothing but link definitions — the references — draws
    // nothing, and an empty chapter would still take a page of its own.
    Ok(chapters
        .into_iter()
        .filter(|chapter| !chapter.pieces.is_empty())
        .collect())
}

/// Every `[id]: url` definition in the manual, as a Markdown block to put in
/// front of each chapter. The sources keep them in one file of their own, but
/// a chapter is parsed on its own, so without this a `[label][id]` reference
/// in another file has nothing to resolve against and stays literal text.
fn link_definitions(texts: &[String]) -> String {
    let mut definitions = BTreeMap::new();
    for markdown in texts {
        collect_link_definitions(
            &parse_markdown(strip_front_matter(markdown)),
            &mut definitions,
        );
    }
    definitions
        .iter()
        .map(|(identifier, url)| format!("[{identifier}]: <{url}>\n"))
        .collect()
}

fn read_chapter(
    path: &Path,
    markdown: &str,
    definitions: &str,
    numbers: &mut [usize; 6],
) -> Result<Chapter> {
    let body = format!("{definitions}\n{}", strip_front_matter(markdown));
    let mut root = parse_markdown(&body);
    resolve_reference_links(&mut root);
    let dir = path.parent().unwrap_or(Path::new("."));

    let mut pieces = Vec::new();
    for node in root.children().map(Vec::as_slice).unwrap_or(&[]) {
        match node {
            Node::Heading(heading) => {
                let plain = plain_text(&inline_spans_of(node));
                let title = number_heading(heading.depth, &plain, numbers);
                let mut blocks = Vec::new();
                render_block_nodes(std::slice::from_ref(node), 0, &mut blocks);
                if let Some(block) = blocks.first_mut() {
                    block.set_text(&title);
                }
                pieces.push(Piece::Heading {
                    depth: heading.depth,
                    title,
                    // The anchor is the heading as written, before it was
                    // numbered: that is what the Markdown links point at.
                    slug: slug_of(&plain),
                    blocks,
                });
            }
            Node::Table(table) => pieces.push(Piece::Table(table_cells(table))),
            // A link definition draws nothing: it has already done its work,
            // resolving the references that point at it.
            Node::Definition(_) => {}
            // A paragraph holding nothing but an image is a figure; a lone
            // `\newpage` is the page break the sources use between chapters.
            Node::Paragraph(paragraph) => match paragraph.children.as_slice() {
                [Node::Image(image)] => pieces.push(Piece::Image(resolve_asset(dir, &image.url))),
                _ if plain_text(&inline_spans_of(node)).trim() == "\\newpage" => {
                    pieces.push(Piece::PageBreak);
                }
                _ => pieces.push(blocks_piece(node)),
            },
            other => pieces.push(blocks_piece(other)),
        }
    }
    Ok(Chapter { pieces })
}

/// Where an image referenced from a chapter actually lives: beside the
/// chapter, or one or two directories up. `doc/manual/en` refers to
/// `images/….png`, which sits in `doc/images`.
fn resolve_asset(dir: &Path, url: &str) -> PathBuf {
    let mut root = Some(dir);
    while let Some(base) = root {
        let candidate = base.join(url);
        if candidate.is_file() {
            return candidate;
        }
        root = base.parent();
    }
    dir.join(url)
}

fn blocks_piece(node: &Node) -> Piece {
    let mut blocks = Vec::new();
    render_block_nodes(std::slice::from_ref(node), 0, &mut blocks);
    Piece::Blocks(blocks)
}

/// Number a heading the way the manual has always been numbered: chapters
/// `1`, sections `1.2`, and so on, resetting every deeper counter.
fn number_heading(depth: u8, text: &str, numbers: &mut [usize; 6]) -> String {
    let index = (depth as usize).min(numbers.len()) - 1;
    numbers[index] += 1;
    for deeper in numbers.iter_mut().skip(index + 1) {
        *deeper = 0;
    }
    let number = numbers[..=index]
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(".");
    format!("{number} {text}")
}

/// A table's cells, row by row, with the header row first.
fn table_cells(table: &Table) -> Vec<Vec<Vec<Span>>> {
    table
        .children
        .iter()
        .filter_map(|row| {
            Some(
                row.children()?
                    .iter()
                    .map(spans_of_node)
                    .collect::<Vec<_>>(),
            )
        })
        .collect()
}

/// A table cell's inline content as spans. Inline code is set bold: the engine
/// embeds no monospaced face, and a key has to stand out from the prose
/// around it.
fn spans_of_node(node: &Node) -> Vec<Span> {
    let children = node.children().map(Vec::as_slice).unwrap_or(&[]);
    if children.len() == 1
        && let Some(Node::Paragraph(paragraph)) = children.first()
    {
        return spans_of(&paragraph.children);
    }
    spans_of(children)
}

fn spans_of(nodes: &[Node]) -> Vec<Span> {
    let mut spans = Vec::new();
    for node in nodes {
        match node {
            Node::InlineCode(code) => spans.push(Span::styled(&code.value, true, false)),
            other => spans.extend(inline_spans_of(other)),
        }
    }
    spans
}

fn plain_text(spans: &[Span]) -> String {
    spans.iter().map(Span::text).collect()
}

/// The title and subtitle for the cover, from the first source's YAML front
/// matter.
fn front_matter(dir: &Path) -> Result<(String, String)> {
    let first = source_files(dir)?.remove(0);
    let text = fs::read_to_string(&first)
        .with_context(|| format!("failed to read {}", first.display()))?;
    let mut title = String::from("pgopr");
    let mut subtitle = String::new();
    for line in text.lines().take_while(|line| !line.starts_with("...")) {
        if let Some(value) = line.strip_prefix("title:") {
            title = value.trim().trim_matches('"').to_string();
        } else if let Some(value) = line.strip_prefix("subtitle:") {
            subtitle = value.trim().trim_matches('"').to_string();
        }
    }
    Ok((title, subtitle))
}

/// Drop a leading `---`/`...` YAML block: it is metadata for the cover, not
/// content, and the Markdown parser would set it as a table.
fn strip_front_matter(markdown: &str) -> &str {
    let trimmed = markdown.trim_start();
    if !trimmed.starts_with("---") {
        return markdown;
    }
    trimmed
        .split_once('\n')
        .and_then(|(_, rest)| {
            rest.find("\n...")
                .or_else(|| rest.find("\n---"))
                .map(|at| &rest[at + 4..])
        })
        .unwrap_or(markdown)
}

/// The cover: the wordmark and its tagline, white on a full-bleed brand page.
fn draw_cover(pdf: &mut Pdf, title: &str, subtitle: &str) {
    pdf.fill_rect(0.0, 0.0, PAGE_WIDTH_MM, PAGE_HEIGHT_MM, BRAND_COLOR);
    let title_width = pdf.text_width_mm(title, true, COVER_TITLE_SIZE);
    pdf.text(
        title,
        true,
        (PAGE_WIDTH_MM - title_width) / 2.0,
        PAGE_HEIGHT_MM / 2.0,
        COVER_TITLE_SIZE,
        WHITE,
    );
    let subtitle_width = pdf.text_width_mm(subtitle, false, COVER_SUBTITLE_SIZE);
    pdf.text(
        subtitle,
        false,
        (PAGE_WIDTH_MM - subtitle_width) / 2.0,
        PAGE_HEIGHT_MM / 2.0 - 18.0,
        COVER_SUBTITLE_SIZE,
        WHITE,
    );
    let footer_width = pdf.text_width_mm(FOOTER, false, 10.0);
    pdf.text(
        FOOTER,
        false,
        (PAGE_WIDTH_MM - footer_width) / 2.0,
        30.0,
        10.0,
        WHITE,
    );
}

/// A contents entry: its depth, its numbered title, and the content page it
/// starts on before the cover and contents are counted in.
struct Entry {
    depth: u8,
    title: String,
    page: usize,
}

/// What a drawing pass learned about where the headings landed: the contents
/// entries, and the page behind each `#anchor` the manual links to.
#[derive(Default)]
struct Outline {
    entries: Vec<Entry>,
    anchors: BTreeMap<String, usize>,
}

/// The anchor a heading answers to, the way the HTML manual names it: folded
/// to lower case, spaces to hyphens, and everything else dropped — so
/// `## Optional external tools` is reached by `](#optional-external-tools)`.
fn slug_of(title: &str) -> String {
    title
        .chars()
        .filter_map(|ch| match ch {
            _ if ch.is_whitespace() => Some('-'),
            '-' | '_' => Some(ch),
            _ if ch.is_alphanumeric() => Some(ch.to_ascii_lowercase()),
            _ => None,
        })
        .collect()
}

const TOC_SIZE: f32 = 10.0;
const TOC_ROW_MM: f32 = TOC_SIZE * 1.7 * PT_TO_MM;

fn toc_page_count(entries: usize) -> usize {
    let rows_per_page = ((CONTENT_TOP_MM - CONTENT_BOTTOM_MM - 12.0) / TOC_ROW_MM) as usize;
    entries.div_ceil(rows_per_page.max(1)).max(1)
}

fn draw_contents(pdf: &mut Pdf, entries: &[Entry], offset: usize) {
    let mut blocks = Vec::new();
    render_block_nodes(
        parse_markdown("# Table of Contents")
            .children()
            .map(Vec::as_slice)
            .unwrap_or(&[]),
        0,
        &mut blocks,
    );
    pdf.draw_blocks(&blocks);

    for entry in entries {
        if pdf.room_left_mm() < TOC_ROW_MM {
            pdf.new_page();
        }
        let y = pdf.cursor() - TOC_ROW_MM;
        pdf.set_cursor(y);
        let indent = (entry.depth.saturating_sub(1)) as f32 * 6.0;
        pdf.text(
            &entry.title,
            entry.depth == 1,
            MARGIN_MM + indent,
            y,
            TOC_SIZE,
            BRAND_COLOR,
        );
        let page = (entry.page + offset).to_string();
        let width = pdf.text_width_mm(&page, false, TOC_SIZE);
        pdf.text(
            &page,
            false,
            PAGE_WIDTH_MM - MARGIN_MM - width,
            y,
            TOC_SIZE,
            BRAND_COLOR,
        );
        pdf.link_to_page(entry.page + offset, y - 1.0, TOC_SIZE * PT_TO_MM + 2.0);
    }
}

/// Draw every chapter, returning where its headings landed. Each chapter
/// opens a fresh page, so these numbers stay true when the cover and contents
/// are added in front of them.
fn draw_chapters(pdf: &mut Pdf, chapters: &[Chapter]) -> Result<Outline> {
    let mut outline = Outline::default();
    let mut first = true;
    for chapter in chapters {
        if !first {
            page_break(pdf);
        }
        first = false;
        for piece in &chapter.pieces {
            match piece {
                Piece::Heading {
                    depth,
                    title,
                    slug,
                    blocks,
                } => {
                    if *depth <= TOC_DEPTH {
                        outline.entries.push(Entry {
                            depth: *depth,
                            title: title.clone(),
                            page: pdf.current_page(),
                        });
                    }
                    outline.anchors.insert(slug.clone(), pdf.current_page());
                    pdf.draw_blocks(blocks);
                }
                Piece::Blocks(blocks) => pdf.draw_blocks(blocks),
                Piece::Table(rows) => draw_table(pdf, rows),
                Piece::Image(path) => match fs::read(path) {
                    Ok(bytes) => pdf.draw_image(&bytes, USABLE_WIDTH_MM)?,
                    Err(error) => {
                        bail!("failed to read {}: {error}", path.display());
                    }
                },
                Piece::PageBreak => page_break(pdf),
            }
        }
    }
    Ok(outline)
}

/// Start a new page, unless nothing has been drawn on this one yet. Every
/// chapter file opens with a `\newpage`, and a chapter already begins on a
/// fresh page, so taking both at face value would leave a blank page between
/// every pair of chapters.
fn page_break(pdf: &mut Pdf) {
    if pdf.cursor() < CONTENT_TOP_MM {
        pdf.new_page();
    }
}

/// Draw a Markdown table as real columns: widths from the content, cells
/// wrapped inside them, the header row in bold over a rule, and a rule under
/// every row. A table that outruns the page continues on the next one under a
/// repeat of its header.
fn draw_table(pdf: &mut Pdf, rows: &[Vec<Vec<Span>>]) {
    let Some(header) = rows.first() else {
        return;
    };
    let widths = column_widths(pdf, rows);
    let row_of = |pdf: &Pdf, cells: &[Vec<Span>]| -> f32 {
        cells
            .iter()
            .zip(&widths)
            .map(|(cell, width)| pdf.spans_height_mm(cell, CODE_SIZE, width - 2.0 * CELL_PAD_MM))
            .fold(0.0_f32, f32::max)
            + 2.0 * CELL_PAD_MM
    };

    let draw_row = |pdf: &mut Pdf, cells: &[Vec<Span>], bold: bool| {
        let height = row_of(pdf, cells);
        if pdf.room_left_mm() < height {
            pdf.new_page();
        }
        let top = pdf.cursor();
        let mut x = MARGIN_MM;
        for (cell, width) in cells.iter().zip(&widths) {
            let spans: Vec<Span> = if bold {
                cell.iter()
                    .map(|span| Span::styled(span.text(), true, false))
                    .collect()
            } else {
                cell.clone()
            };
            pdf.draw_spans_at(
                &spans,
                CODE_SIZE,
                x + CELL_PAD_MM,
                width - 2.0 * CELL_PAD_MM,
                top - CELL_PAD_MM,
                TEXT_COLOR,
            );
            x += width;
        }
        let bottom = top - height;
        pdf.rule(
            MARGIN_MM,
            bottom,
            MARGIN_MM + widths.iter().sum::<f32>(),
            bottom,
            BOX_RULE,
            TABLE_RULE,
        );
        pdf.set_cursor(bottom);
    };

    draw_row(pdf, header, true);
    for cells in rows.iter().skip(1) {
        draw_row(pdf, cells, false);
    }
    pdf.set_cursor(pdf.cursor() - CODE_SIZE * 0.6 * PT_TO_MM);
}

/// The tints derived from the brand colour: a table's rule.
const BOX_RULE: (f32, f32, f32) = (0.851, 0.776, 0.690);

/// Column widths for a table: each column asks for what its longest cell needs
/// on one line, and the excess over the text column is taken back in
/// proportion, so a column of short flags keeps its width and a column of
/// prose gives way.
fn column_widths(pdf: &Pdf, rows: &[Vec<Vec<Span>>]) -> Vec<f32> {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    // What each column would like — its widest cell on one line — and the
    // least it can take before wrapping starts cutting words in half.
    let mut wanted = vec![0.0_f32; columns];
    let mut needed = vec![0.0_f32; columns];
    for (row_index, row) in rows.iter().enumerate() {
        // The header row is drawn bold, so it is measured bold.
        let bold = row_index == 0;
        for (index, cell) in row.iter().enumerate() {
            wanted[index] = wanted[index].max(cell_width(pdf, cell, bold) + 2.0 * CELL_PAD_MM);
            needed[index] = needed[index].max(widest_word(pdf, cell, bold) + 2.0 * CELL_PAD_MM);
        }
    }
    let total: f32 = wanted.iter().sum();
    if total <= USABLE_WIDTH_MM {
        // Spread what is left over the columns, so the table fills the page.
        let extra = (USABLE_WIDTH_MM - total) / columns as f32;
        return wanted.iter().map(|width| width + extra).collect();
    }

    // Too wide for the page. Every column first takes the width of its
    // longest word: a key column is a column of names, and breaking
    // a long flag across three lines to widen prose that did not need
    // it serves nobody. What is left over is then shared out in proportion to
    // how much more each column asked for, which is nearly all prose.
    let floor: f32 = needed.iter().sum();
    if floor >= USABLE_WIDTH_MM {
        // Not even the words fit side by side: scale them down and let them
        // break, which is the best a page this wide can do.
        let scale = USABLE_WIDTH_MM / floor;
        return needed.iter().map(|width| width * scale).collect();
    }
    let slack = USABLE_WIDTH_MM - floor;
    let hunger: f32 = wanted
        .iter()
        .zip(&needed)
        .map(|(wanted, needed)| (wanted - needed).max(0.0))
        .sum();
    needed
        .iter()
        .zip(&wanted)
        .map(|(needed, wanted)| needed + slack * (wanted - needed).max(0.0) / hunger)
        .collect()
}

/// A cell's width set on one line.
fn cell_width(pdf: &Pdf, cell: &[Span], bold: bool) -> f32 {
    cell.iter()
        .map(|span| pdf.text_width_mm(span.text(), bold || span.bold(), CODE_SIZE))
        .sum()
}

/// The width of the longest word in a cell — the narrowest its column can be
/// before wrapping breaks a word rather than a line.
fn widest_word(pdf: &Pdf, cell: &[Span], bold: bool) -> f32 {
    cell.iter()
        .flat_map(|span| {
            span.text()
                .split_whitespace()
                .map(move |word| pdf.text_width_mm(word, bold || span.bold(), CODE_SIZE))
        })
        .fold(0.0_f32, f32::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A manual whose references live in a file of their own, as the real one
    /// does: a chapter that points at `[label][id]`, and the `[id]: url` it
    /// resolves against.
    fn manual_with_references() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("manual");
        std::fs::write(
            dir.path().join("01-introduction.md"),
            "# Introduction\n\nBuilt with [**pgopr**][pgopr].\n",
        )
        .expect("write chapter");
        std::fs::write(
            dir.path().join("99-references.md"),
            "[pgopr]: https://example.invalid/pgopr\n",
        )
        .expect("write references");
        dir
    }

    /// Build a manual and return its bytes and the text drawn on its pages.
    /// A URL that reaches the file without reaching the text is one that was
    /// attached as an annotation.
    fn built(sources: &Path) -> (Vec<u8>, String) {
        let output = sources.join("manual.pdf");
        build_manual(sources, &output).expect("build");
        let bytes = std::fs::read(&output).expect("read pdf");
        let text = pdf_extract::extract_text_from_mem(&bytes).expect("extract text");
        (bytes, text)
    }

    /// Each chapter is parsed on its own, so a reference to a definition in
    /// the references file has nothing to resolve against unless the
    /// definitions are gathered across the whole manual first — and an
    /// unresolved reference is drawn as the literal `[label][id]`.
    #[test]
    fn a_reference_resolves_against_a_definition_in_another_file() {
        let sources = manual_with_references();
        let (bytes, text) = built(sources.path());

        assert!(text.contains("pgopr"), "extracted: {text:?}");
        assert!(!text.contains("[pgopr]"), "extracted: {text:?}");
        // Resolved: the destination is in the file, as a link over the label.
        assert!(contains(&bytes, "https://example.invalid/pgopr"));
    }

    /// A link is drawn as its label alone. The URL belongs to the annotation
    /// over it, not to the text — a reader should see `pgopr`, not
    /// `pgopr (https://…)`.
    #[test]
    fn a_link_is_annotated_rather_than_printed() {
        let sources = manual_with_references();
        let (_, text) = built(sources.path());
        assert!(
            !text.contains("https://example.invalid/pgopr"),
            "extracted: {text:?}"
        );
    }

    /// A link into the manual itself names a heading, not a URL: it becomes a
    /// jump to the page that heading landed on. Left as a URI it would be a
    /// link no viewer could follow.
    #[test]
    fn a_link_to_an_anchor_jumps_to_its_heading() {
        let sources = tempfile::tempdir().expect("manual");
        std::fs::write(
            sources.path().join("01-introduction.md"),
            "# Introduction\n\nSee [the tools](#optional-external-tools).\n",
        )
        .expect("write chapter");
        std::fs::write(
            sources.path().join("02-tools.md"),
            "# Optional external tools\n\nProse.\n",
        )
        .expect("write chapter");

        let (bytes, text) = built(sources.path());
        assert!(text.contains("the tools"), "extracted: {text:?}");
        assert!(!contains(&bytes, "#optional-external-tools"));
        // One jump per contents entry, and one more for the link itself.
        assert_eq!(count(&bytes, "/GoTo"), 3);
    }

    /// Whether `needle` appears in the PDF's bytes: an annotation's URL is
    /// written as a plain string, unlike the compressed page content.
    fn contains(bytes: &[u8], needle: &str) -> bool {
        count(bytes, needle) > 0
    }

    fn count(bytes: &[u8], needle: &str) -> usize {
        bytes
            .windows(needle.len())
            .filter(|window| *window == needle.as_bytes())
            .count()
    }

    /// A key column is a column of names, and a name that breaks across three
    /// lines is unreadable. It keeps the width its longest key needs; the
    /// prose beside it takes what is left and uses more lines instead.
    #[test]
    fn a_key_column_keeps_its_longest_key_on_one_line() {
        let pdf = Pdf::with_footer(MANUAL_HEADER, FOOTER, (FOOTER, FOOTER_URL)).expect("pdf");
        let key = "review_confidence_threshold";
        let markdown = format!(
            "| Key | Description |\n| :-- | :-- |\n| `{key}` | {} |\n",
            "Minimum confidence score for findings, below which they are \
             silently dropped, which is a description long enough to want \
             every millimetre the page can spare."
        );
        let root = parse_markdown(&markdown);
        let table = root
            .children()
            .and_then(|children| {
                children.iter().find_map(|node| match node {
                    Node::Table(table) => Some(table),
                    _ => None,
                })
            })
            .expect("table");

        let widths = column_widths(&pdf, &table_cells(table));
        let needed = pdf.text_width_mm(key, false, CODE_SIZE) + 2.0 * CELL_PAD_MM;
        assert!(widths[0] >= needed, "{widths:?} for a key needing {needed}");
        // And the room it did not take went to the prose.
        assert!(widths[1] > widths[0], "{widths:?}");
    }

    /// The references draw nothing, so they are not a chapter: one that drew
    /// nothing would still take a blank page of its own.
    #[test]
    fn a_file_of_only_link_definitions_is_not_a_chapter() {
        let sources = manual_with_references();
        let chapters = read_chapters(sources.path()).expect("chapters");
        assert_eq!(chapters.len(), 1);
    }

    #[test]
    fn headings_are_numbered_by_depth() {
        let mut numbers = [0usize; 6];
        assert_eq!(
            number_heading(1, "Introduction", &mut numbers),
            "1 Introduction"
        );
        assert_eq!(number_heading(2, "Features", &mut numbers), "1.1 Features");
        assert_eq!(number_heading(3, "A stack", &mut numbers), "1.1.1 A stack");
        assert_eq!(number_heading(2, "Tools", &mut numbers), "1.2 Tools");
        assert_eq!(
            number_heading(1, "Getting started", &mut numbers),
            "2 Getting started"
        );
    }

    #[test]
    fn front_matter_is_not_content() {
        let markdown = "---\ntitle: \"pgopr\"\nsubtitle: \"Advanced\"\n...\n\n# Introduction\n";
        assert_eq!(
            strip_front_matter(markdown).trim_start(),
            "# Introduction\n"
        );
    }

    #[test]
    fn a_document_without_front_matter_is_left_alone() {
        let markdown = "# Introduction\n\nProse.\n";
        assert_eq!(strip_front_matter(markdown), markdown);
    }
}
