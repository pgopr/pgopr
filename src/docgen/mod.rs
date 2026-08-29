/*
 * Eclipse Public License - v 2.0
 *
 *   THE ACCOMPANYING PROGRAM IS PROVIDED UNDER THE TERMS OF THIS ECLIPSE
 *   PUBLIC LICENSE ("AGREEMENT"). ANY USE, REPRODUCTION OR DISTRIBUTION
 *   OF THE PROGRAM CONSTITUTES RECIPIENT'S ACCEPTANCE OF THIS AGREEMENT.
 */

//! The project's PDF engine: the page model, embedded Red Hat Text faces,
//! Markdown-to-block rendering, wrapping, link annotations and image
//! placement.
//!
//! Text is set in **Red Hat Text** (embedded from `assets/fonts`, SIL OFL), so
//! the brand typeface ships in the binary and needs no system fonts. If the
//! embedded font cannot be loaded the export falls back to the closest
//! printpdf-native face (Helvetica), with glyph widths approximated as half an
//! em.
//!
//! Everything is drawn with printpdf: there is no LaTeX and no second
//! toolchain in the PDF path.

pub mod manual;

use anyhow::{Context, Result};
use markdown::{
    ParseOptions,
    mdast::{Code, Heading, Image, Link, List, ListItem, Node, Paragraph},
    to_mdast,
};
use printpdf::{
    Actions, BorderArray, BuiltinFont, Color, Destination, Line, LinePoint, LinkAnnotation, Mm, Op,
    PaintMode, ParsedFont, PdfDocument, PdfFontHandle, PdfPage, PdfSaveOptions, Point, Pt,
    RawImage, Rect, Rgb, TextItem, XObjectTransform,
};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

// --- Embedded brand font (Red Hat Text, SIL OFL — see assets/fonts/LICENSE) ---
const FONT_REGULAR: &[u8] = include_bytes!("../../assets/fonts/RedHatText-Regular.ttf");
const FONT_BOLD: &[u8] = include_bytes!("../../assets/fonts/RedHatText-Bold.ttf");
const FONT_ITALIC: &[u8] = include_bytes!("../../assets/fonts/RedHatText-Italic.ttf");
const FONT_BOLD_ITALIC: &[u8] = include_bytes!("../../assets/fonts/RedHatText-BoldItalic.ttf");

// --- Page geometry (A4, in millimetres) ---
pub(crate) const PAGE_WIDTH_MM: f32 = 210.0;
pub(crate) const PAGE_HEIGHT_MM: f32 = 297.0;
pub(crate) const MARGIN_MM: f32 = 18.0;
pub(crate) const USABLE_WIDTH_MM: f32 = PAGE_WIDTH_MM - 2.0 * MARGIN_MM;

pub(crate) const PT_TO_MM: f32 = 25.4 / 72.0;

pub(crate) const BODY_SIZE: f32 = 10.0;
pub(crate) const CODE_SIZE: f32 = 9.0;
const BAND_TEXT_SIZE: f32 = 11.0;
/// Indent (mm) added per level of list/quote nesting.
const INDENT_MM: f32 = 6.0;

// --- Header/footer bands ---
const HEADER_BAND_MM: f32 = 11.0;
const FOOTER_BAND_MM: f32 = 11.0;
/// Gap between a band and the page content.
const CONTENT_GAP_MM: f32 = 5.0;
/// First content baseline / lowest content baseline.
pub(crate) const CONTENT_TOP_MM: f32 = PAGE_HEIGHT_MM - HEADER_BAND_MM - CONTENT_GAP_MM;
pub(crate) const CONTENT_BOTTOM_MM: f32 = FOOTER_BAND_MM + CONTENT_GAP_MM;

/// The pgopr brand colour (`#336791`, the PostgreSQL blue): the band fill,
/// plus the title and headings.
pub(crate) const BRAND_COLOR: (f32, f32, f32) = (51.0 / 255.0, 103.0 / 255.0, 145.0 / 255.0);
pub(crate) const TEXT_COLOR: (f32, f32, f32) = (0.0, 0.0, 0.0);
pub(crate) const WHITE: (f32, f32, f32) = (1.0, 1.0, 1.0);

/// The resolution an embedded image is placed at before being scaled to the
/// width its page gives it.
const IMAGE_DPI: f32 = 300.0;

/// One run of text with a single style. A style differs by weight and slant,
/// which select a Red Hat Text variant (or its Helvetica fallback).
#[derive(Clone)]
pub(crate) struct Span {
    text: String,
    bold: bool,
    italic: bool,
    /// An explicit RGB fill colour. `None` uses the block's default colour
    /// (brand for headings, black for body).
    color: Option<(f32, f32, f32)>,
    /// The URL this run links to. A Markdown link becomes a run carrying its
    /// destination, which the page turns into a clickable annotation over
    /// exactly the text it drew — so the manual reads `pgopr`, not
    /// `pgopr (https://…)`.
    link: Option<String>,
}

impl Span {
    fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            bold: false,
            italic: false,
            color: None,
            link: None,
        }
    }

    /// A run in a chosen weight and slant, for callers that lay out their own
    /// text (the manual) rather than flow Markdown.
    pub(crate) fn styled(text: impl Into<String>, bold: bool, italic: bool) -> Self {
        Self {
            text: text.into(),
            bold,
            italic,
            color: None,
            link: None,
        }
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn bold(&self) -> bool {
        self.bold
    }
}

/// Parse Markdown into its syntax tree, GFM tables and all. An unparseable
/// document yields an empty root rather than an error: the callers here render
/// what they can.
pub(crate) fn parse_markdown(markdown: &str) -> Node {
    to_mdast(markdown, &ParseOptions::gfm()).unwrap_or_else(|_| {
        Node::Root(markdown::mdast::Root {
            children: Vec::new(),
            position: None,
        })
    })
}

/// Replace reference-style links and images (`[label][id]`, with a matching
/// `[id]: url` definition elsewhere in the document) by their inline
/// equivalents, so they render with their URLs instead of as bare labels.
pub(crate) fn resolve_reference_links(tree: &mut Node) {
    let mut definitions = BTreeMap::new();
    collect_link_definitions(tree, &mut definitions);
    if !definitions.is_empty() {
        replace_reference_nodes(tree, &definitions);
    }
}

/// Every `[id]: url` definition in `node`, by identifier.
pub(crate) fn collect_link_definitions(node: &Node, definitions: &mut BTreeMap<String, String>) {
    if let Node::Definition(definition) = node {
        definitions.insert(definition.identifier.clone(), definition.url.clone());
    }
    if let Some(children) = node.children() {
        for child in children {
            collect_link_definitions(child, definitions);
        }
    }
}

fn replace_reference_nodes(node: &mut Node, definitions: &BTreeMap<String, String>) {
    let Some(children) = node.children_mut() else {
        return;
    };
    for child in children {
        let replacement = match child {
            Node::LinkReference(reference) => definitions.get(&reference.identifier).map(|url| {
                Node::Link(Link {
                    children: std::mem::take(&mut reference.children),
                    position: None,
                    url: url.clone(),
                    title: None,
                })
            }),
            Node::ImageReference(reference) => definitions.get(&reference.identifier).map(|url| {
                Node::Image(Image {
                    position: None,
                    alt: std::mem::take(&mut reference.alt),
                    url: url.clone(),
                    title: None,
                })
            }),
            _ => None,
        };
        if let Some(replacement) = replacement {
            *child = replacement;
        }
        replace_reference_nodes(child, definitions);
    }
}

/// A logical line to be laid out: its styled spans, the font size, the leading
/// indent (mm) of the first and continuation (wrapped) lines, and the gap left
/// after it. `word_wrap` breaks on spaces for prose; code lines wrap hard at the
/// margin so their layout is preserved.
#[derive(Clone)]
pub(crate) struct Block {
    spans: Vec<Span>,
    size: f32,
    indent_mm: f32,
    hanging_mm: f32,
    word_wrap: bool,
    space_after_mm: f32,
    link: Option<String>,
}

impl Block {
    /// Replace the block's text with a single plain run. The manual numbers
    /// its headings (`1.2 Getting Started`) after the Markdown has been
    /// rendered, and this is how the number gets in.
    pub(crate) fn set_text(&mut self, text: &str) {
        self.spans = vec![Span::plain(text)];
    }

    fn paragraph(spans: Vec<Span>) -> Self {
        Self {
            spans,
            size: BODY_SIZE,
            indent_mm: 0.0,
            hanging_mm: 0.0,
            word_wrap: true,
            space_after_mm: BODY_SIZE * 0.5 * PT_TO_MM,
            link: None,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
struct StyledChar {
    ch: char,
    bold: bool,
    italic: bool,
    color: Option<(f32, f32, f32)>,
    /// The index, among the spans this line was laid out from, of the linked
    /// run this character belongs to. Wrapping moves characters between
    /// lines, so the annotation can only be placed once the line is known.
    link: Option<usize>,
}

// --- Markdown to blocks ---

pub(crate) fn render_block_nodes(nodes: &[Node], level: usize, blocks: &mut Vec<Block>) {
    for node in nodes {
        render_block_node(node, level, blocks);
    }
}

fn render_block_node(node: &Node, level: usize, blocks: &mut Vec<Block>) {
    match node {
        Node::Heading(heading) => blocks.push(heading_block(heading, level)),
        Node::Paragraph(paragraph) => {
            blocks.push(paragraph_block(&paragraph.children, level));
        }
        Node::List(list) => render_list(list, level, blocks),
        Node::Code(code) => render_code(code, level, blocks),
        Node::Blockquote(quote) => render_blockquote(&quote.children, level, blocks),
        Node::ThematicBreak(_) => blocks.push(Block {
            spans: vec![Span::plain("—".repeat(40))],
            size: BODY_SIZE,
            indent_mm: level as f32 * INDENT_MM,
            hanging_mm: level as f32 * INDENT_MM,
            word_wrap: false,
            space_after_mm: BODY_SIZE * 0.5 * PT_TO_MM,
            link: None,
        }),
        // Tables are laid out as real columns by the manual itself; one nested
        // inside a list or quote falls back to a monospaced approximation.
        Node::Table(_) => render_table(node, level, blocks),
        // Definitions carry no printable text; anything else is treated as inline.
        Node::Definition(_) => {}
        _ => {
            let spans = inline_spans_of(node);
            if !spans.is_empty() {
                blocks.push(paragraph_block_from_spans(spans, level));
            }
        }
    }
}

fn heading_block(heading: &Heading, level: usize) -> Block {
    let size = match heading.depth {
        1 => BODY_SIZE + 5.0,
        2 => BODY_SIZE + 3.5,
        3 => BODY_SIZE + 2.0,
        4 => BODY_SIZE + 1.0,
        _ => BODY_SIZE + 0.5,
    };
    let mut spans = Vec::new();
    collect_inline(&heading.children, true, false, &mut spans);
    Block {
        spans,
        size,
        indent_mm: level as f32 * INDENT_MM,
        hanging_mm: level as f32 * INDENT_MM,
        word_wrap: true,
        space_after_mm: size * 0.45 * PT_TO_MM,
        link: None,
    }
}

fn paragraph_block(children: &[Node], level: usize) -> Block {
    let mut spans = Vec::new();
    collect_inline(children, false, false, &mut spans);
    paragraph_block_from_spans(spans, level)
}

fn paragraph_block_from_spans(spans: Vec<Span>, level: usize) -> Block {
    Block {
        spans,
        indent_mm: level as f32 * INDENT_MM,
        hanging_mm: level as f32 * INDENT_MM,
        ..Block::paragraph(Vec::new())
    }
}

fn render_list(list: &List, level: usize, blocks: &mut Vec<Block>) {
    let mut number = list.start.unwrap_or(1);
    for child in &list.children {
        let Node::ListItem(item) = child else {
            continue;
        };
        let marker = if list.ordered {
            let marker = format!("{number}.");
            number += 1;
            marker
        } else {
            "•".to_string()
        };
        render_list_item(item, &marker, level, blocks);
    }
}

fn render_list_item(item: &ListItem, marker: &str, level: usize, blocks: &mut Vec<Block>) {
    // Continuation lines hang under the text, past an approximate marker width.
    let marker_allowance = (marker.chars().count() + 1) as f32 * BODY_SIZE * 0.5 * PT_TO_MM;
    let indent_mm = level as f32 * INDENT_MM;
    let mut marker_used = false;
    for child in &item.children {
        match child {
            Node::Paragraph(Paragraph { children, .. }) => {
                let mut spans = Vec::new();
                if !marker_used {
                    spans.push(Span::plain(format!("{marker} ")));
                    marker_used = true;
                }
                collect_inline(children, false, false, &mut spans);
                blocks.push(Block {
                    spans,
                    size: BODY_SIZE,
                    indent_mm,
                    hanging_mm: indent_mm + marker_allowance,
                    word_wrap: true,
                    space_after_mm: BODY_SIZE * 0.3 * PT_TO_MM,
                    link: None,
                });
            }
            Node::List(sub) => render_list(sub, level + 1, blocks),
            other => render_block_node(other, level + 1, blocks),
        }
    }
    if !marker_used {
        blocks.push(Block {
            spans: vec![Span::plain(marker.to_string())],
            size: BODY_SIZE,
            indent_mm,
            hanging_mm: indent_mm + marker_allowance,
            word_wrap: true,
            space_after_mm: BODY_SIZE * 0.3 * PT_TO_MM,
            link: None,
        });
    }
}

fn render_code(code: &Code, level: usize, blocks: &mut Vec<Block>) {
    let base = level as f32 * INDENT_MM + INDENT_MM;
    let lines: Vec<&str> = if code.value.is_empty() {
        vec![""]
    } else {
        code.value.lines().collect()
    };
    let last = lines.len().saturating_sub(1);
    for (index, line) in lines.iter().enumerate() {
        blocks.push(Block {
            spans: vec![Span::plain((*line).to_string())],
            size: CODE_SIZE,
            indent_mm: base,
            hanging_mm: base,
            word_wrap: false,
            space_after_mm: if index == last {
                CODE_SIZE * 0.5 * PT_TO_MM
            } else {
                0.0
            },
            link: None,
        });
    }
}

fn render_blockquote(children: &[Node], level: usize, blocks: &mut Vec<Block>) {
    let start = blocks.len();
    render_block_nodes(children, level, blocks);
    // Prefix every line the quote produced with a "> " marker span.
    for block in &mut blocks[start..] {
        block.spans.insert(0, Span::plain("> "));
        block.hanging_mm += BODY_SIZE * PT_TO_MM;
    }
}

fn render_table(node: &Node, level: usize, blocks: &mut Vec<Block>) {
    let rendered = monospaced_table(node);
    let base = level as f32 * INDENT_MM;
    for line in rendered.lines() {
        blocks.push(Block {
            spans: vec![Span::plain(line.to_string())],
            size: CODE_SIZE,
            indent_mm: base,
            hanging_mm: base,
            word_wrap: false,
            space_after_mm: 0.0,
            link: None,
        });
    }
    if !rendered.is_empty()
        && let Some(last) = blocks.last_mut()
    {
        last.space_after_mm = BODY_SIZE * 0.5 * PT_TO_MM;
    }
}

/// A table as aligned monospaced text: the fallback for tables the caller does
/// not lay out itself (the manual draws top-level tables as real columns).
fn monospaced_table(table: &Node) -> String {
    let Some(children) = table.children() else {
        return String::new();
    };
    let rows: Vec<Vec<String>> = children
        .iter()
        .filter_map(|row| {
            row.children().map(|cells| {
                cells
                    .iter()
                    .map(|cell| {
                        inline_spans_of(cell)
                            .iter()
                            .map(|span| span.text())
                            .collect()
                    })
                    .collect()
            })
        })
        .collect();
    let mut widths: Vec<usize> = Vec::new();
    for row in &rows {
        for (index, cell) in row.iter().enumerate() {
            if widths.len() <= index {
                widths.push(0);
            }
            widths[index] = widths[index].max(cell.chars().count());
        }
    }
    let mut out = String::new();
    for (row_index, row) in rows.iter().enumerate() {
        let line = row
            .iter()
            .enumerate()
            .map(|(index, cell)| format!("{cell:<width$}", width = widths[index]))
            .collect::<Vec<_>>()
            .join(" | ");
        out.push_str(line.trim_end());
        out.push('\n');
        if row_index == 0 && rows.len() > 1 {
            out.push_str(
                &widths
                    .iter()
                    .map(|width| "-".repeat(*width))
                    .collect::<Vec<_>>()
                    .join("-|-"),
            );
            out.push('\n');
        }
    }
    out
}

pub(crate) fn inline_spans_of(node: &Node) -> Vec<Span> {
    let mut spans = Vec::new();
    collect_inline(std::slice::from_ref(node), false, false, &mut spans);
    spans
}

fn collect_inline(nodes: &[Node], bold: bool, italic: bool, out: &mut Vec<Span>) {
    for node in nodes {
        match node {
            Node::Text(text) => push_span(out, &text.value, bold, italic),
            Node::InlineCode(code) => push_span(out, &code.value, bold, italic),
            Node::InlineMath(math) => push_span(out, &math.value, bold, italic),
            Node::Strong(strong) => collect_inline(&strong.children, true, italic, out),
            Node::Emphasis(emphasis) => collect_inline(&emphasis.children, bold, true, out),
            Node::Delete(delete) => collect_inline(&delete.children, bold, italic, out),
            // A link is drawn as its label alone, in the brand colour, with
            // the destination carried on the runs so the page can make them
            // clickable. A link with no label falls back to its URL: there
            // would be nothing to click otherwise.
            Node::Link(link) => {
                let mut label = Vec::new();
                collect_inline(&link.children, bold, italic, &mut label);
                if label.is_empty() {
                    push_span(&mut label, &link.url, bold, italic);
                }
                if !link.url.is_empty() {
                    for span in &mut label {
                        span.color = Some(BRAND_COLOR);
                        span.link = Some(link.url.clone());
                    }
                }
                out.append(&mut label);
            }
            Node::Image(image) => {
                push_span(
                    out,
                    &format!("[image: {}] ({})", image.alt, image.url),
                    bold,
                    italic,
                );
            }
            Node::Break(_) => push_span(out, " ", bold, italic),
            other => {
                if let Some(children) = other.children() {
                    collect_inline(children, bold, italic, out);
                } else if let Some(value) = inline_value(other) {
                    push_span(out, value, bold, italic);
                }
            }
        }
    }
}

fn inline_value(node: &Node) -> Option<&str> {
    match node {
        Node::Html(html) => Some(&html.value),
        Node::Math(math) => Some(&math.value),
        _ => None,
    }
}

/// Append `text` to `out`, turning newlines into spaces and merging into a
/// trailing span when the style matches.
fn push_span(out: &mut Vec<Span>, text: &str, bold: bool, italic: bool) {
    let normalized = text.replace(['\n', '\r'], " ");
    if normalized.is_empty() {
        return;
    }
    match out.last_mut() {
        // A linked run is never extended: the text after a link is not part
        // of it, and merging would put it under the same annotation.
        Some(last) if last.bold == bold && last.italic == italic && last.link.is_none() => {
            last.text.push_str(&normalized)
        }
        _ => out.push(Span {
            text: normalized,
            bold,
            italic,
            color: None,
            link: None,
        }),
    }
}

// --- Fonts & measurement ---

/// The four faces used for drawing, plus the matching width measurer. Either
/// the embedded Red Hat Text family or, if it cannot be loaded, the builtin
/// Helvetica family.
struct DocFonts {
    regular: PdfFontHandle,
    bold: PdfFontHandle,
    italic: PdfFontHandle,
    bold_italic: PdfFontHandle,
    measurer: Measurer,
}

impl DocFonts {
    fn font(&self, bold: bool, italic: bool) -> &PdfFontHandle {
        match (bold, italic) {
            (true, true) => &self.bold_italic,
            (true, false) => &self.bold,
            (false, true) => &self.italic,
            (false, false) => &self.regular,
        }
    }

    fn char_width_mm(&self, ch: char, bold: bool, italic: bool, size: f32) -> f32 {
        self.measurer.char_width_mm(ch, bold, italic, size)
    }

    fn text_width_mm(&self, text: &str, bold: bool, italic: bool, size: f32) -> f32 {
        text.chars()
            .map(|ch| self.char_width_mm(ch, bold, italic, size))
            .sum()
    }
}

/// Measures glyph advances for line breaking and positioning.
enum Measurer {
    /// Real metrics parsed from the embedded Red Hat Text faces, indexed by
    /// `(bold, italic)` as `bold + italic * 2`.
    Embedded(Box<[ttf_parser::Face<'static>; 4]>),
    /// No font metrics available (Helvetica fallback): approximate every glyph
    /// as half an em, which is close to a proportional sans-serif average.
    Approximate,
}

impl Measurer {
    fn char_width_mm(&self, ch: char, bold: bool, italic: bool, size: f32) -> f32 {
        match self {
            Measurer::Embedded(faces) => {
                let face = &faces[usize::from(bold) + usize::from(italic) * 2];
                let units = f32::from(face.units_per_em());
                let advance = face
                    .glyph_index(ch)
                    .and_then(|glyph| face.glyph_hor_advance(glyph))
                    .map_or(units * 0.5, f32::from);
                advance / units * size * PT_TO_MM
            }
            Measurer::Approximate => size * 0.5 * PT_TO_MM,
        }
    }
}

/// Load the embedded Red Hat Text family into `doc`, falling back to the closest
/// printpdf-native face (Helvetica) if it cannot be embedded or parsed.
fn load_fonts(doc: &mut PdfDocument) -> Result<DocFonts> {
    if let Some(fonts) = load_embedded_fonts(doc) {
        return Ok(fonts);
    }
    Ok(DocFonts {
        regular: PdfFontHandle::Builtin(BuiltinFont::Helvetica),
        bold: PdfFontHandle::Builtin(BuiltinFont::HelveticaBold),
        italic: PdfFontHandle::Builtin(BuiltinFont::HelveticaOblique),
        bold_italic: PdfFontHandle::Builtin(BuiltinFont::HelveticaBoldOblique),
        measurer: Measurer::Approximate,
    })
}

/// Parse and register one embedded face, returning its document font handle.
///
/// `ParsedFont::from_bytes` retains the font's source bytes, which
/// `doc.add_font` needs to embed the actual font program (rather than an
/// empty `FontFile`/`FontFile2` stream — a PDF with the right layout but no
/// visible text).
fn add_external_font(doc: &mut PdfDocument, bytes: &'static [u8]) -> Option<PdfFontHandle> {
    let parsed = ParsedFont::from_bytes(bytes, 0, &mut Vec::new())?;
    Some(PdfFontHandle::External(doc.add_font(&parsed)))
}

fn load_embedded_fonts(doc: &mut PdfDocument) -> Option<DocFonts> {
    let regular = add_external_font(doc, FONT_REGULAR)?;
    let bold = add_external_font(doc, FONT_BOLD)?;
    let italic = add_external_font(doc, FONT_ITALIC)?;
    let bold_italic = add_external_font(doc, FONT_BOLD_ITALIC)?;
    let faces = Box::new([
        ttf_parser::Face::parse(FONT_REGULAR, 0).ok()?,
        ttf_parser::Face::parse(FONT_BOLD, 0).ok()?,
        ttf_parser::Face::parse(FONT_ITALIC, 0).ok()?,
        ttf_parser::Face::parse(FONT_BOLD_ITALIC, 0).ok()?,
    ]);
    Some(DocFonts {
        regular,
        bold,
        italic,
        bold_italic,
        measurer: Measurer::Embedded(faces),
    })
}

// --- Layout & PDF output ---

/// Incremental PDF builder: owns the document and fonts, tracks the current
/// page and the content cursor, and draws every page's header/footer bands.
pub(crate) struct Pdf {
    doc: PdfDocument,
    fonts: DocFonts,
    header: String,
    footer: String,
    /// The part of the footer that is underlined and clickable, with the URL it
    /// opens.
    footer_link: (String, String),
    /// Drawing operations accumulated for the page currently being built.
    ops: Vec<Op>,
    /// Pages already finished (flushed by `new_page`/`save`), in order.
    pages: Vec<PdfPage>,
    cursor_y: f32,
    /// The page each `#anchor` names, for documents whose Markdown links
    /// within itself. Empty until a caller that knows its own layout — the
    /// manual, which learns the pages on a first pass — sets it.
    anchors: BTreeMap<String, usize>,
}

impl Pdf {
    /// A document whose footer band carries `footer`. `link` is the part of
    /// the footer to underline and the URL it opens.
    pub(crate) fn with_footer(header: &str, footer: &str, link: (&str, &str)) -> Result<Self> {
        let mut doc = PdfDocument::new("pgopr manual");
        let fonts = load_fonts(&mut doc)?;
        let mut pdf = Pdf {
            doc,
            fonts,
            header: header.to_string(),
            footer: footer.to_string(),
            footer_link: (link.0.to_string(), link.1.to_string()),
            ops: Vec::new(),
            pages: Vec::new(),
            cursor_y: CONTENT_TOP_MM,
            anchors: BTreeMap::new(),
        };
        pdf.draw_furniture();
        Ok(pdf)
    }

    /// Finish the page under construction, pushing its accumulated ops as a new
    /// `PdfPage`.
    fn finish_page(&mut self) {
        let ops = std::mem::take(&mut self.ops);
        self.pages
            .push(PdfPage::new(Mm(PAGE_WIDTH_MM), Mm(PAGE_HEIGHT_MM), ops));
    }

    /// Start a fresh page (with its bands) and reset the content cursor.
    pub(crate) fn new_page(&mut self) {
        self.finish_page();
        self.cursor_y = CONTENT_TOP_MM;
        self.draw_furniture();
    }

    /// The 1-based number of the page currently being drawn — what a
    /// table-of-contents entry for content drawn now should point at.
    pub(crate) fn current_page(&self) -> usize {
        self.pages.len() + 1
    }

    pub(crate) fn draw_blocks(&mut self, blocks: &[Block]) {
        for block in blocks {
            self.draw_block(block);
        }
    }

    fn draw_block(&mut self, block: &Block) {
        let line_height = block.size * 1.35 * PT_TO_MM;
        // Headings (the only blocks larger than the body) carry the brand
        // colour. A span with its own colour overrides this per run;
        // everything else uses the block default.
        let default_color = if block.size > BODY_SIZE {
            BRAND_COLOR
        } else {
            TEXT_COLOR
        };
        for (index, line) in wrap_block(block, &self.fonts).into_iter().enumerate() {
            if self.cursor_y - line_height < CONTENT_BOTTOM_MM {
                self.new_page();
            }
            self.cursor_y -= line_height;
            let indent = if index == 0 {
                block.indent_mm
            } else {
                block.hanging_mm
            };
            draw_line(
                &mut self.ops,
                &line,
                indent,
                block.size,
                self.cursor_y,
                &self.fonts,
                default_color,
            );
            // A linked block attaches a clickable URI annotation over the drawn
            // text (the brand colour already marks it as a link).
            if let Some(url) = &block.link {
                let text: String = line.iter().map(|styled| styled.ch).collect();
                let width = self.fonts.text_width_mm(&text, false, false, block.size);
                self.ops.push(uri_annotation(
                    MARGIN_MM + indent,
                    width,
                    self.cursor_y,
                    block.size,
                    url,
                ));
            }
            self.annotate_links(
                &line,
                &block.spans,
                MARGIN_MM + indent,
                self.cursor_y,
                block.size,
            );
        }
        self.cursor_y -= block.space_after_mm;
    }

    /// Draw a single line of text at `(x, baseline)` in the given colour.
    pub(crate) fn text(
        &mut self,
        text: &str,
        bold: bool,
        x: f32,
        baseline: f32,
        size: f32,
        color: (f32, f32, f32),
    ) {
        let font = self.fonts.font(bold, false).clone();
        emit_text(&mut self.ops, text, &font, size, x, baseline, color);
    }

    pub(crate) fn fill_rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, color: (f32, f32, f32)) {
        let (r, g, b) = color;
        let rect = Rect {
            x: Mm(x0).into(),
            y: Mm(y0).into(),
            width: Mm(x1 - x0).into(),
            height: Mm(y1 - y0).into(),
            mode: Some(PaintMode::Fill),
            winding_order: None,
        };
        self.ops.push(Op::SetFillColor {
            col: Color::Rgb(Rgb::new(r, g, b, None)),
        });
        self.ops.push(Op::DrawPolygon {
            polygon: rect.to_polygon(),
        });
    }

    pub(crate) fn rule(
        &mut self,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        color: (f32, f32, f32),
        thickness: f32,
    ) {
        let (r, g, b) = color;
        self.ops.push(Op::SetOutlineColor {
            col: Color::Rgb(Rgb::new(r, g, b, None)),
        });
        self.ops.push(Op::SetOutlineThickness { pt: Pt(thickness) });
        self.ops.push(Op::DrawLine {
            line: Line {
                points: vec![line_point(x0, y0), line_point(x1, y1)],
                is_closed: false,
            },
        });
    }

    /// Lay a clickable link over each linked run of a line that has just been
    /// drawn. Wrapping decides where a run ends up — a link can be split
    /// across two lines, and a line can carry several — so the annotation is
    /// measured from the characters actually drawn rather than from the span.
    fn annotate_links(
        &mut self,
        line: &[StyledChar],
        spans: &[Span],
        x_mm: f32,
        baseline: f32,
        size: f32,
    ) {
        let mut x = x_mm;
        let mut start = x_mm;
        let mut run: Option<usize> = None;
        for sc in line {
            if sc.link != run {
                self.annotate_run(spans, run, start, x - start, baseline, size);
                run = sc.link;
                start = x;
            }
            x += self.fonts.char_width_mm(sc.ch, sc.bold, sc.italic, size);
        }
        self.annotate_run(spans, run, start, x - start, baseline, size);
    }

    /// One such run: nothing at all unless it came from a linked span.
    fn annotate_run(
        &mut self,
        spans: &[Span],
        run: Option<usize>,
        x_mm: f32,
        width_mm: f32,
        baseline: f32,
        size: f32,
    ) {
        let Some(url) = run
            .and_then(|index| spans.get(index))
            .and_then(|span| span.link.as_deref())
        else {
            return;
        };
        let annotation = link_annotation(x_mm, width_mm, baseline, size, url, &self.anchors);
        self.ops.extend(annotation);
    }

    /// Where each `#anchor` in this document leads (1-based page numbers), so
    /// a Markdown link into the document itself becomes a jump rather than a
    /// URL nothing can open.
    pub(crate) fn set_anchors(&mut self, anchors: BTreeMap<String, usize>) {
        self.anchors = anchors;
    }

    /// Attach an internal "go to page" link spanning the content width at the
    /// given baseline (`page` is 1-based). Used by the table of contents.
    pub(crate) fn link_to_page(&mut self, page: usize, y_mm: f32, height_mm: f32) {
        let rect = Rect {
            x: Mm(MARGIN_MM).into(),
            y: Mm(y_mm).into(),
            width: Mm(USABLE_WIDTH_MM).into(),
            height: Mm(height_mm).into(),
            mode: None,
            winding_order: None,
        };
        self.ops
            .push(annotation(rect, Actions::go_to(top_of_page(page))));
    }

    /// Draw the header and footer bands (brand fill, centered white text) on the
    /// current page, with the footer's linked part underlined and clickable.
    fn draw_furniture(&mut self) {
        self.fill_rect(
            0.0,
            PAGE_HEIGHT_MM - HEADER_BAND_MM,
            PAGE_WIDTH_MM,
            PAGE_HEIGHT_MM,
            BRAND_COLOR,
        );
        self.fill_rect(0.0, 0.0, PAGE_WIDTH_MM, FOOTER_BAND_MM, BRAND_COLOR);

        let cap = BAND_TEXT_SIZE * 0.7 * PT_TO_MM;
        let header_baseline = (PAGE_HEIGHT_MM - HEADER_BAND_MM / 2.0) - cap / 2.0;
        let footer_baseline = FOOTER_BAND_MM / 2.0 - cap / 2.0;

        // Center the band text horizontally.
        let header_width = self
            .fonts
            .text_width_mm(&self.header, true, false, BAND_TEXT_SIZE);
        let header_x = ((PAGE_WIDTH_MM - header_width) / 2.0).max(MARGIN_MM);
        let footer_width = self
            .fonts
            .text_width_mm(&self.footer, false, false, BAND_TEXT_SIZE);
        let footer_x = ((PAGE_WIDTH_MM - footer_width) / 2.0).max(MARGIN_MM);

        let header = self.header.clone();
        let footer = self.footer.clone();
        self.text(
            &header,
            true,
            header_x,
            header_baseline,
            BAND_TEXT_SIZE,
            WHITE,
        );
        self.text(
            &footer,
            false,
            footer_x,
            footer_baseline,
            BAND_TEXT_SIZE,
            WHITE,
        );

        // Underline the footer's linked part and make it clickable.
        let (link_text, link_url) = self.footer_link.clone();
        let link_width = self
            .fonts
            .text_width_mm(&link_text, false, false, BAND_TEXT_SIZE);
        let (wr, wg, wb) = WHITE;
        self.ops.push(Op::SetOutlineColor {
            col: Color::Rgb(Rgb::new(wr, wg, wb, None)),
        });
        self.ops.push(Op::SetOutlineThickness { pt: Pt(0.6) });
        self.ops.push(Op::DrawLine {
            line: Line {
                points: vec![
                    line_point(footer_x, footer_baseline - 1.2),
                    line_point(footer_x + link_width, footer_baseline - 1.2),
                ],
                is_closed: false,
            },
        });
        self.ops.push(Op::LinkAnnotation {
            link: LinkAnnotation::new(
                Rect {
                    x: Mm(footer_x).into(),
                    y: Mm(1.0).into(),
                    width: Mm(link_width).into(),
                    height: Mm(FOOTER_BAND_MM - 2.0).into(),
                    mode: None,
                    winding_order: None,
                },
                Actions::uri(link_url),
                Some(BorderArray::Solid([0.0, 0.0, 0.0])),
                None,
                None,
            ),
        });
    }

    /// Lay out `spans` in `size` from `x_mm`, wrapped to `width_mm`, with the
    /// first baseline `y_mm` and returning the baseline just past the last
    /// line. Callers that place text in boxes and columns of their own measure
    /// with [`Self::spans_height_mm`] and draw here.
    pub(crate) fn draw_spans_at(
        &mut self,
        spans: &[Span],
        size: f32,
        x_mm: f32,
        width_mm: f32,
        y_mm: f32,
        color: (f32, f32, f32),
    ) -> f32 {
        let chars = spans_chars(spans);
        let line_height = size * 1.35 * PT_TO_MM;
        let mut y = y_mm;
        for line in wrap(&chars, width_mm, width_mm, true, &self.fonts, size) {
            y -= line_height;
            draw_line(
                &mut self.ops,
                &line,
                x_mm - MARGIN_MM,
                size,
                y,
                &self.fonts,
                color,
            );
            self.annotate_links(&line, spans, x_mm, y, size);
        }
        y
    }

    /// What [`Self::draw_spans_at`] would consume vertically (mm), so a caller
    /// can size a box before drawing into it.
    pub(crate) fn spans_height_mm(&self, spans: &[Span], size: f32, width_mm: f32) -> f32 {
        let lines = wrap(
            &spans_chars(spans),
            width_mm,
            width_mm,
            true,
            &self.fonts,
            size,
        );
        lines.len() as f32 * size * 1.35 * PT_TO_MM
    }

    /// The width (mm) `text` occupies in the given weight and size.
    pub(crate) fn text_width_mm(&self, text: &str, bold: bool, size: f32) -> f32 {
        self.fonts.text_width_mm(text, bold, false, size)
    }

    /// The baseline the next content would be drawn at, and a way to move it.
    pub(crate) fn cursor(&self) -> f32 {
        self.cursor_y
    }

    pub(crate) fn set_cursor(&mut self, y: f32) {
        self.cursor_y = y;
    }

    /// Room left on the page below the cursor (mm).
    pub(crate) fn room_left_mm(&self) -> f32 {
        self.cursor_y - CONTENT_BOTTOM_MM
    }

    /// Draw a PNG centred on the text column, scaled to fit `max_width_mm` and
    /// whatever height is left, and advance the cursor past it. The image
    /// starts a fresh page when what remains is too short to be worth using.
    pub(crate) fn draw_image(&mut self, bytes: &[u8], max_width_mm: f32) -> Result<()> {
        let mut warnings = Vec::new();
        let image = RawImage::decode_from_bytes(bytes, &mut warnings)
            .map_err(|error| anyhow::anyhow!("could not decode the image: {error}"))?;
        let (pixel_width, pixel_height) = (image.width as f32, image.height as f32);

        // Images are placed by their natural size at `IMAGE_DPI`, then scaled
        // to the width they are given.
        let natural_width = pixel_width / IMAGE_DPI * 25.4;
        let natural_height = pixel_height / IMAGE_DPI * 25.4;
        let mut width = natural_width.min(max_width_mm);
        let mut height = natural_height * width / natural_width;

        let full_page = CONTENT_TOP_MM - CONTENT_BOTTOM_MM;
        if height > self.room_left_mm() && height <= full_page {
            self.new_page();
        }
        if height > full_page {
            height = full_page;
            width = natural_width * height / natural_height;
        }

        let id = self.doc.add_image(&image);
        let x = MARGIN_MM + (USABLE_WIDTH_MM - width) / 2.0;
        let bottom = self.cursor_y - height;
        self.ops.push(Op::UseXobject {
            id,
            transform: XObjectTransform {
                translate_x: Some(Mm(x).into()),
                translate_y: Some(Mm(bottom).into()),
                scale_x: Some(width / natural_width),
                scale_y: Some(height / natural_height),
                dpi: Some(IMAGE_DPI),
                ..Default::default()
            },
        });
        self.cursor_y = bottom - BODY_SIZE * PT_TO_MM;
        Ok(())
    }

    pub(crate) fn save(mut self, path: &Path) -> Result<()> {
        self.finish_page();
        let pages = std::mem::take(&mut self.pages);
        self.doc.with_pages(pages);
        // printpdf's font subsetting (glyph renumbering via allsorts) scrambles
        // the embedded outlines for the Red Hat Text faces — the PDF ends up
        // with the right layout and correct copy/paste text, but wrong or
        // missing glyphs on screen. Embedding the full (unsubset) face avoids
        // that renumbering step; it costs a few embedded fonts' worth of file
        // size, negligible next to a manual.
        let opts = PdfSaveOptions {
            subset_fonts: false,
            ..PdfSaveOptions::default()
        };
        let bytes = self.doc.save(&opts, &mut Vec::new());
        let file =
            File::create(path).with_context(|| format!("failed to create {}", path.display()))?;
        BufWriter::new(file)
            .write_all(&bytes)
            .with_context(|| format!("failed to write {}", path.display()))?;
        Ok(())
    }
}

/// Wrap a block into visual lines at the page width.
fn wrap_block(block: &Block, fonts: &DocFonts) -> Vec<Vec<StyledChar>> {
    let first_width = (USABLE_WIDTH_MM - block.indent_mm).max(1.0);
    let cont_width = (USABLE_WIDTH_MM - block.hanging_mm).max(1.0);
    wrap(
        &spans_chars(&block.spans),
        first_width,
        cont_width,
        block.word_wrap,
        fonts,
        block.size,
    )
}

/// The styled characters of a run of spans. Each character remembers which
/// span it came from when that span is linked, so the annotation can be laid
/// over it once wrapping has settled which line it fell on.
fn spans_chars(spans: &[Span]) -> Vec<StyledChar> {
    spans
        .iter()
        .enumerate()
        .flat_map(|(index, span)| {
            span.text.chars().map(move |ch| StyledChar {
                ch,
                bold: span.bold,
                italic: span.italic,
                color: span.color,
                link: span.link.as_ref().map(|_| index),
            })
        })
        .collect()
}

/// Break a styled line into visual lines that fit the page width (mm). The first
/// line uses `first_width`, the rest `cont_width`. With `word_wrap`, breaks fall
/// on spaces (an over-long word is split hard); without it, every line is cut
/// hard at the width.
fn wrap(
    chars: &[StyledChar],
    first_width: f32,
    cont_width: f32,
    word_wrap: bool,
    fonts: &DocFonts,
    size: f32,
) -> Vec<Vec<StyledChar>> {
    let width_of = |sc: &StyledChar| fonts.char_width_mm(sc.ch, sc.bold, sc.italic, size);

    let mut lines: Vec<Vec<StyledChar>> = Vec::new();
    let mut line: Vec<StyledChar> = Vec::new();
    let mut line_width = 0.0_f32;
    let mut last_space: Option<usize> = None;
    let mut budget = first_width;

    let mut push_line = |line: &mut Vec<StyledChar>, carry: &[StyledChar]| {
        lines.push(std::mem::take(line));
        line.extend_from_slice(carry);
    };

    for &sc in chars {
        let w = width_of(&sc);
        if !line.is_empty() && line_width + w > budget {
            match (word_wrap, last_space) {
                // Break at the last space: the text after it carries to the next line.
                (true, Some(at)) if at > 0 => {
                    let carry: Vec<StyledChar> = line[at + 1..].to_vec();
                    line.truncate(at);
                    push_line(&mut line, &carry);
                }
                // No usable break point: cut hard before this character.
                _ => push_line(&mut line, &[]),
            }
            budget = cont_width;
            line_width = line.iter().map(width_of).sum();
            last_space = line.iter().rposition(|c| c.ch == ' ');
        }
        if sc.ch == ' ' {
            last_space = Some(line.len());
        }
        line.push(sc);
        line_width += w;
    }
    if !line.is_empty() || lines.is_empty() {
        lines.push(line);
    }
    lines
}

/// The style of one drawn run: weight, slant, and resolved RGB fill colour.
/// Runs break whenever any of these change.
type RunStyle = (bool, bool, (f32, f32, f32));

fn draw_line(
    ops: &mut Vec<Op>,
    line: &[StyledChar],
    indent_mm: f32,
    size: f32,
    y: f32,
    fonts: &DocFonts,
    default_color: (f32, f32, f32),
) {
    let mut x = MARGIN_MM + indent_mm;
    let mut run = String::new();
    let mut run_style: Option<RunStyle> = None;

    for sc in line {
        // A span's own colour wins; everything else takes the block default
        // resolved here, so the run carries a concrete colour.
        let style = (sc.bold, sc.italic, sc.color.unwrap_or(default_color));
        if run_style != Some(style) {
            x = flush_run(ops, &mut run, x, y, size, run_style, fonts);
            run_style = Some(style);
        }
        run.push(sc.ch);
    }
    flush_run(ops, &mut run, x, y, size, run_style, fonts);
}

/// The annotation for one drawn link run: a jump to the page an in-document
/// `#anchor` names, or a URI for anything else. An anchor this document does
/// not define gets nothing — a link to a destination that is not here would
/// be a link that does nothing when clicked.
fn link_annotation(
    x_mm: f32,
    width_mm: f32,
    baseline: f32,
    size: f32,
    url: &str,
    anchors: &BTreeMap<String, usize>,
) -> Option<Op> {
    let Some(anchor) = url.strip_prefix('#') else {
        return Some(uri_annotation(x_mm, width_mm, baseline, size, url));
    };
    let page = *anchors.get(anchor)?;
    Some(page_annotation(x_mm, width_mm, baseline, size, page))
}

/// A clickable URI over the text drawn from `x_mm` at `baseline`, tall enough
/// to cover the line it sits on.
fn uri_annotation(x_mm: f32, width_mm: f32, baseline: f32, size: f32, url: &str) -> Op {
    annotation(
        line_rect(x_mm, width_mm, baseline, size),
        Actions::uri(url.to_string()),
    )
}

/// A jump to the top of `page` (1-based) over the text drawn from `x_mm` at
/// `baseline`.
fn page_annotation(x_mm: f32, width_mm: f32, baseline: f32, size: f32, page: usize) -> Op {
    annotation(
        line_rect(x_mm, width_mm, baseline, size),
        Actions::go_to(top_of_page(page)),
    )
}

/// The box a one-line annotation covers: the drawn run, with a little air
/// above and below so the whole glyph height is clickable.
fn line_rect(x_mm: f32, width_mm: f32, baseline: f32, size: f32) -> Rect {
    Rect {
        x: Mm(x_mm).into(),
        y: Mm(baseline - 1.5).into(),
        width: Mm(width_mm).into(),
        height: Mm(size * PT_TO_MM + 2.5).into(),
        mode: None,
        winding_order: None,
    }
}

/// A destination showing `page` (1-based) from its top edge.
fn top_of_page(page: usize) -> Destination {
    Destination::Xyz {
        page,
        left: Some(0.0),
        top: Some(PAGE_HEIGHT_MM / PT_TO_MM),
        zoom: None,
    }
}

fn annotation(rect: Rect, actions: Actions) -> Op {
    Op::LinkAnnotation {
        link: LinkAnnotation::new(
            rect,
            actions,
            Some(BorderArray::Solid([0.0, 0.0, 0.0])),
            None,
            None,
        ),
    }
}

/// Draw `run` at `(x, y)` (mm) in its style's colour and return the x just past
/// it.
fn flush_run(
    ops: &mut Vec<Op>,
    run: &mut String,
    x: f32,
    y: f32,
    size: f32,
    style: Option<RunStyle>,
    fonts: &DocFonts,
) -> f32 {
    if run.is_empty() {
        return x;
    }
    let (bold, italic, color) = style.unwrap_or((false, false, TEXT_COLOR));
    let width = fonts.text_width_mm(run, bold, italic, size);
    emit_text(ops, run, fonts.font(bold, italic), size, x, y, color);
    run.clear();
    x + width
}

/// A non-bezier line vertex at `(x, y)` millimetres.
fn line_point(x: f32, y: f32) -> LinePoint {
    LinePoint {
        p: Point::new(Mm(x), Mm(y)),
        bezier: false,
    }
}

/// Append the ops drawing one absolutely-positioned text run: a self-contained
/// text section (`BT … ET`) whose single `Td` places the baseline at `(x, y)`
/// millimetres from the page's bottom-left. Each run is its own section so the
/// text matrix resets and `Td` acts as an absolute move.
fn emit_text(
    ops: &mut Vec<Op>,
    text: &str,
    font: &PdfFontHandle,
    size: f32,
    x: f32,
    y: f32,
    color: (f32, f32, f32),
) {
    if text.is_empty() {
        return;
    }
    let (r, g, b) = color;
    ops.push(Op::StartTextSection);
    ops.push(Op::SetFillColor {
        col: Color::Rgb(Rgb::new(r, g, b, None)),
    });
    ops.push(Op::SetFont {
        font: font.clone(),
        size: Pt(size),
    });
    ops.push(Op::SetTextCursor {
        pos: Point::new(Mm(x), Mm(y)),
    });
    ops.push(Op::ShowText {
        items: vec![TextItem::Text(text.to_string())],
    });
    ops.push(Op::EndTextSection);
}
