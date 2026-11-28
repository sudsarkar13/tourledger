//! Draws a report as an A4 PDF, in Rust so every platform gets the same file.
//! The on-screen preview is the same laid-out pages, so it cannot differ.
//! The embedded DejaVu Sans covers Latin text and the rupee sign; characters it
//! has no shape for (Indian scripts, for one) are written as "?".

use krilla::Document;
use krilla::color::rgb;
use krilla::geom::{PathBuilder, Point, Rect, Size, Transform};
use krilla::image::Image;
use krilla::num::NormalizedF32;
use krilla::page::PageSettings;
use krilla::paint::Fill;
use krilla::text::{Font, TextDirection};

use std::collections::BTreeMap;

use serde::Serialize;

use crate::error::{Error, Result};
use crate::money;
use crate::report::{Bill, Line, Report};

/// Bill pictures (JPEG) by hash.
pub type Pictures = BTreeMap<String, Vec<u8>>;

pub const WIDTH: f32 = 595.0;
pub const HEIGHT: f32 = 842.0;
const MARGIN: f32 = 40.0;
const BODY: f32 = 9.0;

struct Face {
    font: Font,
    data: &'static [u8],
}

impl Face {
    fn load(data: &'static [u8]) -> Result<Self> {
        let font = Font::new(data.to_vec().into(), 0)
            .ok_or_else(|| Error::msg("The report font could not be read."))?;
        Ok(Self { font, data })
    }

    /// Replaces characters the font cannot draw, and measures the result.
    fn fit(&self, text: &str, size: f32) -> (String, f32) {
        let Ok(face) = ttf_parser::Face::parse(self.data, 0) else {
            return (text.to_owned(), 0.0);
        };
        let scale = size / f32::from(face.units_per_em());
        let mut out = String::new();
        let mut width = 0.0;
        for c in text.chars().map(|c| if c.is_control() { ' ' } else { c }) {
            let (c, glyph) = match face.glyph_index(c) {
                Some(glyph) => (c, Some(glyph)),
                None => ('?', face.glyph_index('?')),
            };
            let advance = glyph.and_then(|g| face.glyph_hor_advance(g)).unwrap_or(0);
            width += f32::from(advance) * scale;
            out.push(c);
        }
        (out, width)
    }

    /// Breaks text into lines no wider than `max`.
    fn wrap(&self, text: &str, size: f32, max: f32) -> Vec<String> {
        let mut lines = Vec::new();
        let mut line = String::new();
        for word in text.split_whitespace() {
            let candidate = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if self.fit(&candidate, size).1 <= max {
                line = candidate;
                continue;
            }
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            // A single word wider than the column is cut where it must be.
            for c in word.chars() {
                let candidate = format!("{line}{c}");
                if !line.is_empty() && self.fit(&candidate, size).1 > max {
                    lines.push(std::mem::replace(&mut line, c.to_string()));
                } else {
                    line = candidate;
                }
            }
        }
        if !line.is_empty() {
            lines.push(line);
        }
        if lines.is_empty() {
            lines.push(String::new());
        }
        lines
    }
}

#[derive(Clone, Copy)]
enum Align {
    Left,
    Right,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Mark {
    Text {
        x: f32,
        y: f32,
        text: String,
        size: f32,
        bold: bool,
        grey: u8,
        /// As drawn in the PDF, so a preview in another font can match it.
        width: f32,
    },
    Image {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        hash: String,
    },
    Box {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        grey: u8,
    },
}

/// Lays marks out on pages first, so page numbers can say "of how many".
struct Layout {
    regular: Face,
    bold: Face,
    pages: Vec<Vec<Mark>>,
    y: f32,
}

impl Layout {
    fn face(&self, bold: bool) -> &Face {
        if bold { &self.bold } else { &self.regular }
    }

    fn push(&mut self, mark: Mark) {
        if let Some(page) = self.pages.last_mut() {
            page.push(mark);
        }
    }

    /// Starts a new page if `height` does not fit. True when it did.
    fn room(&mut self, height: f32) -> bool {
        if self.y + height <= HEIGHT - MARGIN - 16.0 {
            return false;
        }
        self.pages.push(Vec::new());
        self.y = MARGIN;
        true
    }

    // A drawing call: position, words and look. A struct would only rename these.
    #[allow(clippy::too_many_arguments)]
    fn text(&mut self, x: f32, y: f32, text: &str, size: f32, bold: bool, grey: u8, align: Align) {
        if text.is_empty() {
            return;
        }
        let (text, width) = self.face(bold).fit(text, size);
        let x = match align {
            Align::Left => x,
            Align::Right => x - width,
        };
        self.push(Mark::Text {
            x,
            y,
            text,
            size,
            bold,
            grey,
            width,
        });
    }

    fn rule(&mut self, grey: u8) {
        let y = self.y;
        self.push(Mark::Box {
            x: MARGIN,
            y,
            w: WIDTH - 2.0 * MARGIN,
            h: 0.7,
            grey,
        });
    }

    fn band(&mut self, height: f32) {
        let y = self.y;
        self.push(Mark::Box {
            x: MARGIN,
            y,
            w: WIDTH - 2.0 * MARGIN,
            h: height,
            grey: 236,
        });
    }

    fn heading(&mut self, title: &str) {
        self.room(60.0);
        self.y += 18.0;
        let y = self.y;
        self.text(MARGIN, y, title, 11.5, true, 0, Align::Left);
        self.y += 6.0;
        self.rule(120);
        self.y += 4.0;
    }

    /// A label and an amount on one line, the amount at the right edge.
    fn amount_row(&mut self, label: &str, amount: i64, bold: bool) {
        self.room(16.0);
        self.y += 13.0;
        let y = self.y;
        self.text(MARGIN + 6.0, y, label, BODY, bold, 0, Align::Left);
        let value = format!("₹ {}", money::indian(amount));
        self.text(WIDTH - MARGIN - 6.0, y, &value, BODY, bold, 0, Align::Right);
        self.y += 4.0;
    }

    /// As `amount_row`, for a label that may need more than one line.
    fn long_amount_row(&mut self, label: &str, amount: i64) {
        let lines = self.regular.wrap(label, BODY, WIDTH - 2.0 * MARGIN - 100.0);
        self.room(lines.len() as f32 * 12.0 + 6.0);
        let value = format!("₹ {}", money::indian(amount));
        for (i, line) in lines.iter().enumerate() {
            self.y += 12.0;
            let y = self.y;
            self.text(MARGIN + 6.0, y, line, BODY, false, 0, Align::Left);
            if i == 0 {
                self.text(
                    WIDTH - MARGIN - 6.0,
                    y,
                    &value,
                    BODY,
                    false,
                    0,
                    Align::Right,
                );
            }
        }
        self.y += 5.0;
    }

    /// Two columns of "label: value".
    fn pairs(&mut self, pairs: &[(String, String)]) {
        let half = (WIDTH - 2.0 * MARGIN) / 2.0;
        for row in pairs.chunks(2) {
            let cells: Vec<Vec<String>> = row
                .iter()
                .map(|(_, value)| self.regular.wrap(value, BODY, half - 90.0))
                .collect();
            let lines = cells.iter().map(Vec::len).max().unwrap_or(1);
            self.room(lines as f32 * 12.0 + 4.0);
            for (column, ((label, _), values)) in row.iter().zip(cells).enumerate() {
                let x = MARGIN + column as f32 * half;
                let top = self.y;
                self.text(x, top + 11.0, label, BODY, false, 100, Align::Left);
                for (i, value) in values.iter().enumerate() {
                    self.text(
                        x + 82.0,
                        top + 11.0 + i as f32 * 12.0,
                        value,
                        BODY,
                        false,
                        0,
                        Align::Left,
                    );
                }
            }
            self.y += lines as f32 * 12.0 + 2.0;
        }
    }
}

/// Left edge and width of each column: date, particulars, for, paid by, bill.
const COLUMNS: [(f32, f32); 5] = [
    (MARGIN + 4.0, 56.0),
    (MARGIN + 62.0, 190.0),
    (MARGIN + 258.0, 90.0),
    (MARGIN + 352.0, 52.0),
    (MARGIN + 408.0, 40.0),
];

fn table_header(layout: &mut Layout) {
    layout.band(16.0);
    let y = layout.y + 11.0;
    for ((x, _), label) in COLUMNS
        .iter()
        .zip(["Date", "Particulars", "For", "Paid by", "Bill"])
    {
        layout.text(*x, y, label, 8.0, true, 0, Align::Left);
    }
    layout.text(
        WIDTH - MARGIN - 6.0,
        y,
        "Amount (₹)",
        8.0,
        true,
        0,
        Align::Right,
    );
    layout.y += 16.0;
}

fn short_date(iso: &str) -> String {
    chrono::NaiveDate::parse_from_str(iso, "%Y-%m-%d")
        .map_or_else(|_| iso.to_owned(), |d| d.format("%d %b %y").to_string())
}

fn table_line(layout: &mut Layout, line: &Line, grey: u8) {
    let cells = [
        short_date(&line.date),
        line.particulars.clone(),
        line.people.clone(),
        line.paid_by.clone(),
        std::iter::once(line.bill_no.clone())
            .chain(line.bills.iter().map(|n| format!("#{n}")))
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
    ];
    let wrapped: Vec<Vec<String>> = cells
        .iter()
        .zip(COLUMNS)
        .map(|(text, (_, width))| layout.regular.wrap(text, BODY - 0.5, width))
        .collect();
    let height = wrapped.iter().map(Vec::len).max().unwrap_or(1) as f32 * 11.0 + 5.0;
    if layout.room(height) {
        table_header(layout);
    }
    let top = layout.y;
    for (lines, (x, _)) in wrapped.iter().zip(COLUMNS) {
        for (i, text) in lines.iter().enumerate() {
            layout.text(
                x,
                top + 11.0 + i as f32 * 11.0,
                text,
                BODY - 0.5,
                false,
                grey,
                Align::Left,
            );
        }
    }
    let amount = money::indian(line.amount);
    layout.text(
        WIDTH - MARGIN - 6.0,
        top + 11.0,
        &amount,
        BODY,
        false,
        grey,
        Align::Right,
    );
    layout.y += height;
    layout.rule(220);
}

fn compose(report: &Report, layout: &mut Layout) {
    layout.y += 14.0;
    let y = layout.y;
    layout.text(MARGIN, y, &report.title, 17.0, true, 0, Align::Left);
    layout.y += 4.0;
    let subtitle = if report.company.is_empty() {
        report.tour.clone()
    } else {
        format!("{}  ·  {}", report.company, report.tour)
    };
    // Long company and tour names wrap instead of running off the page.
    for line in layout.regular.wrap(&subtitle, 10.5, WIDTH - 2.0 * MARGIN) {
        layout.y += 14.0;
        let y = layout.y;
        layout.text(MARGIN, y, &line, 10.5, false, 60, Align::Left);
    }
    layout.y += 8.0;
    layout.rule(0);
    layout.y += 6.0;
    layout.pairs(&report.claimant);
    layout.y += 4.0;
    layout.pairs(&report.details);

    layout.heading("Summary by category");
    for group in &report.groups {
        if !group.lines.is_empty() {
            // Without the entry tables, this is where a bill is traced from.
            let bills: Vec<String> = group
                .lines
                .iter()
                .flat_map(|l| &l.bills)
                .map(|n| format!("#{n}"))
                .collect();
            let label = if report.detail || bills.is_empty() {
                group.name.clone()
            } else {
                format!("{}  (bills {})", group.name, bills.join(" "))
            };
            layout.amount_row(&label, group.total, false);
            layout.rule(220);
        }
    }
    layout.amount_row(
        if report.is_claim {
            "Total claimed"
        } else {
            "Total spent"
        },
        report.total,
        true,
    );

    for group in report
        .groups
        .iter()
        .filter(|g| report.detail && !g.lines.is_empty())
    {
        layout.heading(&group.name);
        table_header(layout);
        for line in &group.lines {
            table_line(layout, line, 0);
        }
        layout.amount_row(
            &format!("Total {}", group.name.to_lowercase()),
            group.total,
            true,
        );
    }

    if report.company_total > 0 {
        layout.heading("Paid directly by the company (not claimed)");
        if report.detail {
            table_header(layout);
            for line in report.groups.iter().flat_map(|g| &g.company_lines) {
                table_line(layout, line, 90);
            }
        } else {
            for group in report.groups.iter().filter(|g| g.company_total > 0) {
                layout.amount_row(&group.name, group.company_total, false);
                layout.rule(220);
            }
        }
        layout.amount_row("Total paid by company", report.company_total, true);
    }

    if !report.stages.is_empty() {
        layout.heading("Spent at each stage of the trip");
        for stage in &report.stages {
            layout.amount_row(&stage.name, stage.total, false);
            layout.rule(220);
        }
    }

    if !report.by_person.is_empty() {
        layout.heading("Spent for each person");
        for (name, value) in &report.by_person {
            layout.amount_row(name, *value, false);
            layout.rule(220);
        }
    }

    let proof = report.proof && report.proof_summary.all.count > 0;
    if proof {
        proof_summary(report, layout);
    }

    if report.is_claim {
        // Decided before the heading, so the heading and sums stay on one page.
        layout.room(140.0 + report.money.len() as f32 * 17.0);
        layout.heading("Settlement");
        layout.amount_row("Total claimed", report.total, false);
        for (date, label, amount) in &report.money {
            let sign = if label.contains("returned") {
                "Add"
            } else {
                "Less"
            };
            layout.amount_row(
                &format!("{sign}: {label}, {}", short_date(date)),
                *amount,
                false,
            );
        }
        layout.rule(0);
        layout.amount_row(&report.net_label, report.net.abs(), true);
        layout.y += 14.0;
        let words = layout
            .regular
            .wrap(&report.net_words, BODY, WIDTH - 2.0 * MARGIN);
        for line in words {
            let y = layout.y;
            layout.text(MARGIN + 6.0, y, &line, BODY, false, 60, Align::Left);
            layout.y += 12.0;
        }
        if report.allowance > 0 {
            let note = format!(
                "Bonus or allowance of ₹ {} received from the company, not adjusted here.",
                money::indian(report.allowance)
            );
            layout.y += 4.0;
            let y = layout.y;
            layout.text(MARGIN + 6.0, y, &note, BODY, false, 60, Align::Left);
            layout.y += 12.0;
        }
        if !report.bills.is_empty() {
            let note = format!(
                "Supporting bills attached: {}, on the following pages.",
                report.bills.len()
            );
            let y = layout.y + 8.0;
            layout.text(MARGIN + 6.0, y, &note, BODY, false, 60, Align::Left);
            layout.y += 14.0;
        }
    }

    // Last, so it reads as the statement the name under it makes.
    if report.proof && !report.unproved.is_empty() {
        declaration(report, layout);
    }
}

/// How my payments were made, and how many have a picture.
fn proof_summary(report: &Report, layout: &mut Layout) {
    let p = &report.proof_summary;
    let payments = |n: usize| format!("{n} {}", if n == 1 { "payment" } else { "payments" });
    layout.heading("How I paid, and the pictures attached");
    for mode in &p.modes {
        let label = format!(
            "{}: {}, {} with a picture",
            mode.name,
            payments(mode.all.count),
            mode.pictured.count
        );
        layout.amount_row(&label, mode.all.total, false);
        layout.rule(220);
    }
    layout.amount_row(
        &format!("With a picture: {}", payments(p.pictured.count)),
        p.pictured.total,
        true,
    );
    layout.amount_row(
        &format!(
            "Without a picture: {}",
            payments(p.all.count - p.pictured.count)
        ),
        p.all.total - p.pictured.total,
        true,
    );
}

fn declaration(report: &Report, layout: &mut Layout) {
    layout.heading("Declaration: payments with no bill or payment screenshot attached");
    for line in layout
        .regular
        .wrap(&report.statement, BODY, WIDTH - 2.0 * MARGIN)
    {
        layout.room(12.0);
        layout.y += 12.0;
        let y = layout.y;
        layout.text(MARGIN + 6.0, y, &line, BODY, false, 0, Align::Left);
    }
    layout.y += 6.0;
    for line in &report.unproved {
        let mut label = format!(
            "{}  ·  {}  ·  {}",
            short_date(&line.date),
            line.category,
            line.mode
        );
        // What was bought, so that it can be approved without a picture.
        label.push_str("  ·  ");
        label.push_str(&line.particulars);
        layout.long_amount_row(&label, line.amount);
        layout.rule(220);
    }
    layout.amount_row("Total without a picture", report.unproved_total, true);
    layout.room(34.0);
    layout.y += 6.0;
    let by = [
        ("Name", report.declared_by.as_str()),
        ("Date", report.declared_on.as_str()),
    ];
    for (label, value) in by.into_iter().filter(|(_, v)| !v.is_empty()) {
        layout.y += 12.0;
        let y = layout.y;
        let text = format!("{label}: {value}");
        layout.text(MARGIN + 6.0, y, &text, BODY, false, 0, Align::Left);
    }
}

const GUTTER: f32 = 12.0;
/// A picture this many times taller than wide gets a half page, so that a long
/// receipt stays readable.
const TALL: f32 = 2.2;

/// Which of the four quarters (two across, two down) a bill takes, if they fit.
fn place(taken: &[bool; 4], bill: &Bill) -> Option<Vec<usize>> {
    let free = |cells: &[usize]| cells.iter().all(|c| !taken[*c]);
    let options: &[&[usize]] = if bill.full_page {
        &[&[0, 1, 2, 3]]
    } else if bill.height as f32 > bill.width as f32 * TALL {
        &[&[0, 2], &[1, 3]]
    } else {
        &[&[0], &[1], &[2], &[3]]
    };
    options
        .iter()
        .find(|cells| free(cells))
        .map(|cells| cells.to_vec())
}

/// Pages of pictures: four to a page, each labelled with its transaction.
fn bill_pages(bills: &[Bill], layout: &mut Layout) {
    let top = MARGIN + 28.0;
    let cell_w = (WIDTH - 2.0 * MARGIN - GUTTER) / 2.0;
    let cell_h = (HEIGHT - MARGIN - 16.0 - top - GUTTER) / 2.0;
    let mut taken = [true; 4];

    for bill in bills {
        let cells = place(&taken, bill).unwrap_or_else(|| {
            layout.pages.push(Vec::new());
            taken = [false; 4];
            let y = MARGIN + 14.0;
            layout.text(MARGIN, y, "Supporting bills", 13.0, true, 0, Align::Left);
            place(&taken, bill).unwrap_or_else(|| vec![0])
        });
        cells.iter().for_each(|c| taken[*c] = true);

        let columns = if cells.contains(&0) && cells.contains(&1) {
            2.0
        } else {
            1.0
        };
        let rows = if cells.len() as f32 / columns > 1.0 {
            2.0
        } else {
            1.0
        };
        let x = MARGIN + (cells[0] % 2) as f32 * (cell_w + GUTTER);
        let y = top + (cells[0] / 2) as f32 * (cell_h + GUTTER);
        let w = cell_w * columns + GUTTER * (columns - 1.0);
        let h = cell_h * rows + GUTTER * (rows - 1.0);

        for (bx, by, bw, bh) in [
            (x, y, w, 0.7),
            (x, y + h, w, 0.7),
            (x, y, 0.7, h),
            (x + w, y, 0.7, h),
        ] {
            layout.push(Mark::Box {
                x: bx,
                y: by,
                w: bw,
                h: bh,
                grey: 190,
            });
        }
        // Measured bold, the wider face used here, so no line can spill over.
        let label = layout.bold.wrap(&bill.label, 8.5, w - 12.0);
        let shown = label.len().min(3);
        for (i, line) in label.iter().take(shown).enumerate() {
            layout.text(
                x + 6.0,
                y + 12.0 + i as f32 * 11.0,
                line,
                8.5,
                i == 0,
                0,
                Align::Left,
            );
        }

        let area_y = y + 12.0 + shown as f32 * 11.0;
        let (area_w, area_h) = (w - 12.0, y + h - 6.0 - area_y);
        if !bill.present {
            let note = "This picture is not on this device yet.";
            layout.text(x + 6.0, area_y + 20.0, note, 8.5, false, 110, Align::Left);
            continue;
        }
        let scale = (area_w / bill.width as f32).min(area_h / bill.height as f32);
        let (draw_w, draw_h) = (bill.width as f32 * scale, bill.height as f32 * scale);
        layout.push(Mark::Image {
            x: x + 6.0 + (area_w - draw_w) / 2.0,
            y: area_y,
            w: draw_w,
            h: draw_h,
            hash: bill.hash.clone(),
        });
    }
}

fn fill(grey: u8) -> Fill {
    Fill {
        paint: rgb::Color::new(grey, grey, grey).into(),
        opacity: NormalizedF32::ONE,
        rule: Default::default(),
    }
}

fn lay_out(report: &Report) -> Result<Layout> {
    let mut layout = Layout {
        regular: Face::load(dejavu::sans::regular())?,
        bold: Face::load(dejavu::sans::bold())?,
        pages: vec![Vec::new()],
        y: MARGIN,
    };
    compose(report, &mut layout);
    bill_pages(&report.bills, &mut layout);

    let total = layout.pages.len();
    for i in 0..total {
        let (text, width) = layout
            .regular
            .fit(&format!("Page {} of {total}", i + 1), 8.0);
        layout.pages[i].push(Mark::Text {
            x: WIDTH - MARGIN - width,
            y: HEIGHT - MARGIN + 8.0,
            text,
            size: 8.0,
            bold: false,
            grey: 110,
            width,
        });
    }

    Ok(layout)
}

/// The pages as they will be drawn, for showing before saving. A picture that
/// could not be read is simply absent, as in the PDF.
pub fn preview(report: &Report, pictures: &Pictures) -> Result<Vec<Vec<Mark>>> {
    let mut pages = lay_out(report)?.pages;
    for page in &mut pages {
        page.retain(
            |mark| !matches!(mark, Mark::Image { hash, .. } if !pictures.contains_key(hash)),
        );
    }
    Ok(pages)
}

pub fn render(report: &Report, pictures: &Pictures) -> Result<Vec<u8>> {
    let layout = lay_out(report)?;
    let failed = || Error::msg("The PDF could not be made.");
    let settings = PageSettings::from_wh(WIDTH, HEIGHT).ok_or_else(failed)?;
    let mut document = Document::new();
    for marks in &layout.pages {
        let mut page = document.start_page_with(settings.clone());
        let mut surface = page.surface();
        for mark in marks {
            match mark {
                Mark::Text {
                    x,
                    y,
                    text,
                    size,
                    bold,
                    grey,
                    width: _,
                } => {
                    surface.set_fill(Some(fill(*grey)));
                    surface.draw_text(
                        Point::from_xy(*x, *y),
                        layout.face(*bold).font.clone(),
                        *size,
                        text,
                        false,
                        TextDirection::Auto,
                    );
                }
                Mark::Image { x, y, w, h, hash } => {
                    let Some(bytes) = pictures.get(hash) else {
                        continue;
                    };
                    let image = Image::from_jpeg(bytes.clone().into(), true)
                        .map_err(|_| Error::msg("A bill picture could not be added."))?;
                    if let Some(size) = Size::from_wh(*w, *h) {
                        surface.push_transform(&Transform::from_translate(*x, *y));
                        surface.draw_image(image, size);
                        surface.pop();
                    }
                }
                Mark::Box { x, y, w, h, grey } => {
                    let mut path = PathBuilder::new();
                    if let Some(rect) = Rect::from_xywh(*x, *y, *w, *h) {
                        path.push_rect(rect);
                    }
                    if let Some(path) = path.finish() {
                        surface.set_fill(Some(fill(*grey)));
                        surface.draw_path(&path);
                    }
                }
            }
        }
        surface.finish();
        page.finish();
    }
    document.finish().map_err(|_| failed())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bill(width: u32, height: u32, full_page: bool) -> Bill {
        Bill {
            number: 1,
            hash: "h".into(),
            label: "Bill".into(),
            width,
            height,
            full_page,
            present: true,
        }
    }

    fn pages_for(bills: &[Bill]) -> Vec<usize> {
        let mut layout = Layout {
            regular: Face::load(dejavu::sans::regular()).unwrap(),
            bold: Face::load(dejavu::sans::bold()).unwrap(),
            pages: Vec::new(),
            y: MARGIN,
        };
        bill_pages(bills, &mut layout);
        layout
            .pages
            .iter()
            .map(|p| p.iter().filter(|m| matches!(m, Mark::Image { .. })).count())
            .collect()
    }

    #[test]
    fn four_small_bills_share_a_page() {
        let small = || bill(900, 1200, false);
        assert_eq!(
            pages_for(&[small(), small(), small(), small(), small()]),
            [4, 1]
        );
    }

    #[test]
    fn long_receipts_and_full_pages_get_more_room() {
        let tall = || bill(400, 1600, false);
        assert_eq!(pages_for(&[tall(), tall(), tall()]), [2, 1]);
        // A long receipt beside two small ones, then a bill needing a page.
        let small = || bill(900, 900, false);
        assert_eq!(
            pages_for(&[tall(), small(), small(), bill(900, 900, true), small()]),
            [3, 1, 1]
        );
    }

    #[test]
    fn long_labels_stay_inside_their_cell() {
        let mut layout = Layout {
            regular: Face::load(dejavu::sans::regular()).unwrap(),
            bold: Face::load(dejavu::sans::bold()).unwrap(),
            pages: Vec::new(),
            y: MARGIN,
        };
        let mut long = bill(900, 900, false);
        long.label =
            "Bill 4: 16 Nov 2026, Event and tickets, Exhibition entry passes, ₹ 500.00".into();
        bill_pages(&[long.clone(), long], &mut layout);
        let half = MARGIN + (WIDTH - 2.0 * MARGIN - GUTTER) / 2.0;
        for mark in layout.pages.iter().flatten() {
            if let Mark::Text { x, width, .. } = mark {
                let edge = if *x < half { half } else { WIDTH - MARGIN };
                assert!(x + width <= edge, "text ends at {} past {edge}", x + width);
            }
        }
    }

    #[test]
    fn nothing_is_drawn_outside_the_page() {
        let mut layout = Layout {
            regular: Face::load(dejavu::sans::regular()).unwrap(),
            bold: Face::load(dejavu::sans::bold()).unwrap(),
            pages: Vec::new(),
            y: MARGIN,
        };
        let shapes = [
            bill(4000, 300, false),
            bill(300, 4000, false),
            bill(10, 10, true),
        ];
        bill_pages(&shapes, &mut layout);
        for mark in layout.pages.iter().flatten() {
            if let Mark::Image { x, y, w, h, .. } = mark {
                assert!(*x >= 0.0 && *y >= 0.0 && x + w <= WIDTH && y + h <= HEIGHT);
            }
        }
    }
}
