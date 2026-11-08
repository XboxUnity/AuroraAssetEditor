use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::image_change::ImageChange;
use super::image_files::PNG_EXTENSION;
use super::pixel_comparison::PixelComparison;
use super::run_folder::{BASELINE_COPY_FOLDER, DIFF_FOLDER, IMAGES_FOLDER};
use super::suite_report::{ImageReport, SuiteReport};

pub const CONTACT_SHEET_FILE: &str = "index.html";

const PAGE_TITLE: &str = "UI screenshots";
/// The page's CSS. The page is mid grey, so the dark and the light theme both stand out from it.
const STYLE: &str = r#"
*{box-sizing:border-box}
body{margin:0;background:#808080;color:#101215;font:15px/1.5 "Segoe UI",system-ui,-apple-system,"Helvetica Neue",Arial,sans-serif}
main{max-width:2160px;margin:0 auto;padding:32px 24px 72px}
a{color:#0b4fb3}
code,.name{font-family:ui-monospace,"Cascadia Mono",Consolas,Menlo,monospace}
.summary{background:#f2f3f5;border-radius:12px;padding:24px 28px;box-shadow:0 2px 10px rgba(0,0,0,.25)}
.title{display:flex;align-items:center;gap:14px;flex-wrap:wrap;margin:0 0 16px}
h1{margin:0;font-size:28px;font-weight:650;letter-spacing:-.01em}
.verdict{padding:2px 14px;border-radius:999px;font-weight:700;color:#fff}
.verdict-passed{background:#1b7f3b}
.verdict-failed{background:#c62828}
.counts{display:flex;flex-wrap:wrap;gap:6px 28px;list-style:none;margin:0;padding:0}
.counts strong{font-size:22px;font-variant-numeric:tabular-nums;margin-right:6px}
.notice{margin:16px 0 0;padding:10px 14px;background:#fff4d1;border-left:4px solid #c99700;border-radius:4px}
.summary h2{font-size:15px;margin:22px 0 6px}
.summary ul.list{margin:0;padding-left:22px}
.summary ul.list li{margin:3px 0}
.section{margin-top:56px}
.section>h2{font-size:24px;margin:0 0 20px;padding-bottom:8px;border-bottom:2px solid rgba(0,0,0,.28)}
.screen{margin:0 0 44px}
.screen h3{margin:0;font-size:17px;font-weight:650}
.area-title{font-size:13px;font-weight:400;color:#2b2e33}
.meta{margin:0 0 16px;color:#2b2e33}
.jumps{margin:16px 0 0;font-weight:600}
.jumps a{margin-right:20px}
details.unchanged-area{margin:0 0 20px}
details.unchanged-area>summary{cursor:pointer;font-size:17px;font-weight:650;margin-bottom:16px}
.description{margin:2px 0 14px;max-width:90ch;color:#16181c}
.themes{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:28px;align-items:start}
@media (max-width:900px){.themes{grid-template-columns:minmax(0,1fr)}}
.shot{margin:0;border-radius:6px}
.shot:target{outline:3px solid #ffd400;outline-offset:8px}
.shot figcaption{display:flex;align-items:center;gap:10px;margin-bottom:8px;font-size:13px}
.theme{font-weight:700;text-transform:uppercase;letter-spacing:.08em}
.badge{padding:1px 9px;border-radius:999px;font-size:12px;font-weight:650;color:#fff}
.badge-unchanged{background:#3a3e45;color:#e7e8ea}
.badge-changed{background:#c62828}
.badge-unreadable-baseline{background:#a85400}
.badge-removed{background:#5d4037}
.badge-new{background:#1b7f3b}
.badge-error{background:#7b1fa2}
.outcome{margin:0 0 10px;font-size:13px}
.panel+.panel{margin-top:14px}
.panel-caption{margin:0 0 4px;font-size:12px;font-weight:650;letter-spacing:.02em}
.shot img{display:block;width:100%;height:auto;background:#6e6e6e;box-shadow:0 1px 5px rgba(0,0,0,.45)}
.error{margin:0;padding:12px 14px;background:#fbeaea;color:#5c0f0f;border-left:4px solid #7b1fa2;border-radius:4px;white-space:pre-wrap;overflow-wrap:anywhere;font:13px/1.45 ui-monospace,"Cascadia Mono",Consolas,Menlo,monospace}
"#;
const REMOVED_ID: &str = "removed";
const PAGE_END: &str = "</main>\n</body>\n</html>\n";

/// Writes the page the user reviews the UI from: counts, then the screens that need a look
/// (errors, changes, new screens, removed images) and last the unchanged ones, folded away. Images
/// are linked by paths relative to `output`, so the folder stands on its own in a zip.
pub fn write_contact_sheet(report: &SuiteReport, output: &Path) -> io::Result<PathBuf> {
    fs::create_dir_all(output)?;
    let path = output.join(CONTACT_SHEET_FILE);
    fs::write(&path, contact_sheet_html(report))?;
    Ok(path)
}

fn contact_sheet_html(report: &SuiteReport) -> String {
    let mut html = format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{PAGE_TITLE}</title>\n<style>{STYLE}</style>\n</head>\n<body>\n<main>\n"
    );
    let screens = shown_screens(report);
    push_header(&mut html, report, &section_links(report, &screens));
    for section in [Section::Errors, Section::Changed, Section::New] {
        push_section(&mut html, section, &screens_in(&screens, section));
    }
    push_removed(&mut html, report);
    push_unchanged(&mut html, &screens_in(&screens, Section::Unchanged));
    html.push_str(PAGE_END);
    html
}

/// Where a screen is listed: the first section any of its images belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Errors,
    Changed,
    New,
    Unchanged,
}

impl Section {
    fn id(self) -> &'static str {
        match self {
            Section::Errors => "errors",
            Section::Changed => "changed",
            Section::New => "new",
            Section::Unchanged => "unchanged",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Section::Errors => "Errors",
            Section::Changed => "Changed",
            Section::New => "New",
            Section::Unchanged => "Unchanged",
        }
    }

    fn of(screen: &ShownScreen<'_>) -> Section {
        let has = |wanted: fn(&ImageChange) -> bool| {
            screen.images.iter().any(|image| wanted(&image.change))
        };
        if has(|change| matches!(change, ImageChange::Error(_))) {
            Section::Errors
        } else if has(|change| {
            matches!(
                change,
                ImageChange::Changed(_) | ImageChange::BaselineUnreadable(_)
            )
        }) {
            Section::Changed
        } else if has(|change| matches!(change, ImageChange::New)) {
            Section::New
        } else {
            Section::Unchanged
        }
    }
}

/// A screen as the sheet shows it: its images, one per theme, in catalogue order.
struct ShownScreen<'a> {
    area: &'static str,
    images: Vec<&'a ImageReport>,
}

/// Every screen of the catalogue with each image once. An image name the catalogue repeats
/// already fails the run with a problem; showing the image again would repeat its id on the page.
fn shown_screens(report: &SuiteReport) -> Vec<ShownScreen<'_>> {
    let mut shown = BTreeSet::new();
    let mut screens = Vec::new();
    for area in &report.areas {
        let images: Vec<&ImageReport> = area
            .images
            .iter()
            .filter(|image| shown.insert(image.image.as_str()))
            .collect();
        for screen_images in images.chunk_by(|first, second| first.screen == second.screen) {
            screens.push(ShownScreen {
                area: area.title,
                images: screen_images.to_vec(),
            });
        }
    }
    screens
}

fn screens_in<'a, 'r>(
    screens: &'a [ShownScreen<'r>],
    section: Section,
) -> Vec<&'a ShownScreen<'r>> {
    screens
        .iter()
        .filter(|screen| Section::of(screen) == section)
        .collect()
}

/// The header's links to the sections the page has, each with its id and count.
fn section_links(report: &SuiteReport, screens: &[ShownScreen<'_>]) -> Vec<(&'static str, String)> {
    let mut links = Vec::new();
    for section in [
        Section::Errors,
        Section::Changed,
        Section::New,
        Section::Unchanged,
    ] {
        let count = screens_in(screens, section).len();
        if count > 0 {
            links.push((section.id(), format!("{} ({count})", section.title())));
        }
        if section == Section::New && !report.removed.is_empty() {
            links.push((REMOVED_ID, format!("Removed ({})", report.removed.len())));
        }
    }
    links
}

fn push_header(html: &mut String, report: &SuiteReport, links: &[(&str, String)]) {
    let (verdict_class, verdict) = if report.passed() {
        ("verdict-passed", "Passed")
    } else {
        ("verdict-failed", "Failed")
    };
    html.push_str("<header class=\"summary\">\n");
    html.push_str(&format!(
        "<div class=\"title\"><h1>{PAGE_TITLE}</h1><span class=\"verdict {verdict_class}\">{verdict}</span></div>\n"
    ));
    html.push_str(&format!(
        "<p class=\"meta\">{}: {}, rendered {}. {}</p>\n",
        escape(&report.platform),
        escape(&report.label),
        escape(&report.rendered),
        escape(&compared_with(report))
    ));
    push_counts(html, report);
    if let Some(text) = &report.filter {
        html.push_str(&format!(
            "<p class=\"notice\">Filtered to screens containing <code>{}</code>: other screens were not rendered, and removed screens were not looked for.</p>\n",
            escape(text)
        ));
    }
    push_list(html, "Problems", &report.problems);
    push_list(html, "Notes", &report.notes);
    if !links.is_empty() {
        html.push_str("<p class=\"jumps\">");
        for (id, text) in links {
            html.push_str(&format!("<a href=\"#{id}\">{}</a>", escape(text)));
        }
        html.push_str("</p>\n");
    }
    html.push_str("</header>\n");
}

fn compared_with(report: &SuiteReport) -> String {
    match &report.baseline {
        Some(baseline) => format!(
            "Compared with {}, rendered {}.",
            baseline.label, baseline.rendered
        ),
        None => "No baseline: every image is new.".to_owned(),
    }
}

fn push_counts(html: &mut String, report: &SuiteReport) {
    let counts = report.counts();
    let entries = [
        (counts.screens, "screens"),
        (counts.images, "images"),
        (counts.changed, "changed"),
        (counts.new, "new"),
        (counts.removed, "removed"),
        (counts.unchanged, "unchanged"),
        (counts.unreadable_baselines, "unreadable baselines"),
        (counts.errors, "errors"),
    ];
    html.push_str("<ul class=\"counts\">\n");
    for (count, label) in entries {
        html.push_str(&format!("<li><strong>{count}</strong>{label}</li>\n"));
    }
    html.push_str("</ul>\n");
}

fn push_list(html: &mut String, heading: &str, items: &[String]) {
    if items.is_empty() {
        return;
    }
    html.push_str(&format!(
        "<h2>{}</h2>\n<ul class=\"list\">\n",
        escape(heading)
    ));
    for item in items {
        html.push_str(&format!("<li>{}</li>\n", escape(item)));
    }
    html.push_str("</ul>\n");
}

/// A section with its screens; nothing when there are none.
fn push_section(html: &mut String, section: Section, screens: &[&ShownScreen<'_>]) {
    if screens.is_empty() {
        return;
    }
    push_section_start(html, section.id(), section.title());
    for screen in screens {
        push_screen(html, screen);
    }
    html.push_str("</section>\n");
}

/// The unchanged screens, one closed `<details>` per area.
fn push_unchanged(html: &mut String, screens: &[&ShownScreen<'_>]) {
    if screens.is_empty() {
        return;
    }
    push_section_start(html, Section::Unchanged.id(), Section::Unchanged.title());
    for area_screens in screens.chunk_by(|first, second| first.area == second.area) {
        let count = area_screens.len();
        let noun = if count == 1 { "screen" } else { "screens" };
        html.push_str(&format!(
            "<details class=\"unchanged-area\">\n<summary>{} ({count} {noun})</summary>\n",
            escape(area_screens[0].area)
        ));
        for screen in area_screens {
            push_screen(html, screen);
        }
        html.push_str("</details>\n");
    }
    html.push_str("</section>\n");
}

fn push_section_start(html: &mut String, id: &str, title: &str) {
    html.push_str(&format!(
        "<section class=\"section\" id=\"{id}\">\n<h2>{title}</h2>\n"
    ));
}

/// One screen: its name, area and description, then its images (one per theme) side by side.
fn push_screen(html: &mut String, screen: &ShownScreen<'_>) {
    let Some(first) = screen.images.first() else {
        return;
    };
    html.push_str(&format!(
        "<article class=\"screen\">\n<h3><span class=\"name\">{}</span> <span class=\"area-title\">{}</span></h3>\n<p class=\"description\">{}</p>\n<div class=\"themes\">\n",
        escape(first.screen),
        escape(screen.area),
        escape(first.description)
    ));
    for image in &screen.images {
        push_image(html, image);
    }
    html.push_str("</div>\n</article>\n");
}

fn push_image(html: &mut String, image: &ImageReport) {
    let name = image.image.as_str();
    html.push_str(&format!(
        "<figure class=\"shot\" id=\"{}\">\n<figcaption><span class=\"theme\">{}</span>{}</figcaption>\n",
        escape(name),
        escape(image.theme),
        badge(&image.change)
    ));
    match &image.change {
        ImageChange::Unchanged => push_panel(html, None, IMAGES_FOLDER, name),
        ImageChange::Changed(comparison) => push_comparison(html, comparison, name),
        ImageChange::New => {
            push_outcome(html, "New: the baseline has no such image.");
            push_panel(html, None, IMAGES_FOLDER, name);
        }
        ImageChange::BaselineUnreadable(reason) => {
            push_outcome(html, reason);
            push_panel(html, None, IMAGES_FOLDER, name);
        }
        // An error can come before the screenshot was written or from writing it, so the file may
        // be missing or incomplete; an error shows its message only, never an image.
        ImageChange::Error(message) => {
            html.push_str(&format!("<pre class=\"error\">{}</pre>\n", escape(message)));
        }
    }
    html.push_str("</figure>\n");
}

/// Every removed image with its baseline copy; nothing when none was removed.
fn push_removed(html: &mut String, report: &SuiteReport) {
    if report.removed.is_empty() {
        return;
    }
    push_section_start(html, REMOVED_ID, "Removed");
    html.push_str("<div class=\"themes\">\n");
    for name in &report.removed {
        html.push_str(&format!(
            "<figure class=\"shot\" id=\"{}\">\n<figcaption><span class=\"name\">{}</span><span class=\"badge badge-removed\">removed</span></figcaption>\n",
            escape(name),
            escape(name)
        ));
        push_outcome(html, "Removed: the current run has no such image.");
        push_panel(html, None, BASELINE_COPY_FOLDER, name);
        html.push_str("</figure>\n");
    }
    html.push_str("</div>\n</section>\n");
}

/// The baseline's image, the screenshot of this run and the diff, each captioned.
fn push_comparison(html: &mut String, comparison: &PixelComparison, image: &str) {
    push_outcome(html, &comparison.to_string());
    push_panel(html, Some("Baseline"), BASELINE_COPY_FOLDER, image);
    push_panel(html, Some("Current"), IMAGES_FOLDER, image);
    push_panel(
        html,
        Some("Diff: baseline, current, changes in magenta"),
        DIFF_FOLDER,
        image,
    );
}

fn push_outcome(html: &mut String, text: &str) {
    html.push_str(&format!("<p class=\"outcome\">{}</p>\n", escape(text)));
}

/// An image linking to its full-size PNG.
fn push_panel(html: &mut String, caption: Option<&str>, folder: &str, image: &str) {
    let path = escape(&format!("{folder}/{image}.{PNG_EXTENSION}"));
    html.push_str("<div class=\"panel\">\n");
    if let Some(caption) = caption {
        html.push_str(&format!(
            "<p class=\"panel-caption\">{}</p>\n",
            escape(caption)
        ));
    }
    html.push_str(&format!(
        "<a href=\"{path}\"><img src=\"{path}\" alt=\"{}\" loading=\"lazy\"></a>\n</div>\n",
        escape(&format!("{image} ({folder})"))
    ));
}

fn badge(change: &ImageChange) -> String {
    let (class, label) = match change {
        ImageChange::Unchanged => ("unchanged", "unchanged"),
        ImageChange::Changed(_) => ("changed", "changed"),
        ImageChange::New => ("new", "new"),
        ImageChange::BaselineUnreadable(_) => ("unreadable-baseline", "unreadable baseline"),
        ImageChange::Error(_) => ("error", "error"),
    };
    format!("<span class=\"badge badge-{class}\">{label}</span>")
}

/// Text placed in the page, as element content or in a quoted attribute.
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            other => escaped.push(other),
        }
    }
    escaped
}
