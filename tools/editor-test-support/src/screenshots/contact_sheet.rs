use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::pixel_comparison::PixelComparison;
use super::reference_store::{
    ACTUAL_FOLDER, DIFF_FOLDER, ImageStatus, PNG_EXTENSION, REFERENCE_COPY_FOLDER,
};
use super::suite_report::{AreaReport, ImageReport, SuiteReport};

pub const CONTACT_SHEET_FILE: &str = "index.html";

const PAGE_TITLE: &str = "UI screenshots";
/// Mid grey, so the dark and the light theme both stand out from the page.
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
.all-clear{margin:22px 0 0;color:#1b5e30;font-weight:600}
.area{margin-top:56px}
.area>h2{font-size:24px;margin:0 0 20px;padding-bottom:8px;border-bottom:2px solid rgba(0,0,0,.28)}
.screen{margin:0 0 44px}
.screen h3{margin:0;font-size:17px;font-weight:650}
.description{margin:2px 0 14px;max-width:90ch;color:#16181c}
.themes{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:28px;align-items:start}
@media (max-width:900px){.themes{grid-template-columns:minmax(0,1fr)}}
.shot{margin:0;border-radius:6px}
.shot:target{outline:3px solid #ffd400;outline-offset:8px}
.shot figcaption{display:flex;align-items:center;gap:10px;margin-bottom:8px;font-size:13px}
.theme{font-weight:700;text-transform:uppercase;letter-spacing:.08em}
.badge{padding:1px 9px;border-radius:999px;font-size:12px;font-weight:650;color:#fff}
.badge-matches{background:#3a3e45;color:#e7e8ea}
.badge-differs{background:#c62828}
.badge-missing{background:#a85400}
.badge-updated{background:#1d4ed8}
.badge-new{background:#1b7f3b}
.badge-error{background:#7b1fa2}
.outcome{margin:0 0 10px;font-size:13px}
.panel+.panel{margin-top:14px}
.panel-caption{margin:0 0 4px;font-size:12px;font-weight:650;letter-spacing:.02em}
.shot img{display:block;width:100%;height:auto;background:#6e6e6e;box-shadow:0 1px 5px rgba(0,0,0,.45)}
.error{margin:0;padding:12px 14px;background:#fbeaea;color:#5c0f0f;border-left:4px solid #7b1fa2;border-radius:4px;white-space:pre-wrap;overflow-wrap:anywhere;font:13px/1.45 ui-monospace,"Cascadia Mono",Consolas,Menlo,monospace}
"#;
const PAGE_END: &str = "</main>\n</body>\n</html>\n";

/// Writes the page the user approves the UI from: counts, everything that needs a look, then
/// every screen with its dark and light image side by side. Images are linked by paths relative
/// to `output`, so the folder stands on its own in a zip.
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
    push_summary(&mut html, report);
    for area in &report.areas {
        push_area(&mut html, area);
    }
    html.push_str(PAGE_END);
    html
}

fn push_summary(html: &mut String, report: &SuiteReport) {
    let (verdict_class, verdict) = if report.passed() {
        ("verdict-passed", "Passed")
    } else {
        ("verdict-failed", "Failed")
    };
    html.push_str("<header class=\"summary\">\n");
    html.push_str(&format!(
        "<div class=\"title\"><h1>{PAGE_TITLE}</h1><span class=\"verdict {verdict_class}\">{verdict}</span></div>\n"
    ));
    push_counts(html, report);
    if let Some(text) = &report.filter {
        html.push_str(&format!(
            "<p class=\"notice\">Filtered to screens containing <code>{}</code>: other screens and orphan references were not checked.</p>\n",
            escape(text)
        ));
    }
    push_list(html, "Problems", &report.problems);
    push_list(
        html,
        "References without a screen in the catalogue",
        &report.orphans,
    );
    push_list(
        html,
        "Removed references without a screen",
        &report.removed_orphans,
    );
    push_needs_a_look(html, report);
    html.push_str("</header>\n");
}

fn push_counts(html: &mut String, report: &SuiteReport) {
    let counts = report.counts();
    let entries = [
        (counts.screens, "screens"),
        (counts.images, "images"),
        (counts.matching, "matching"),
        (counts.differing, "differing"),
        (counts.missing, "missing"),
        (counts.updated, "updated"),
        (counts.new, "new"),
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

/// Links to every image that is not plain matching, so a long sheet never hides one.
fn push_needs_a_look(html: &mut String, report: &SuiteReport) {
    let flagged: Vec<&ImageReport> = report
        .images()
        .filter(|image| image.status != ImageStatus::Matches)
        .collect();
    if flagged.is_empty() {
        if report.images().next().is_some() {
            html.push_str("<p class=\"all-clear\">Every image matches its reference.</p>\n");
        }
        return;
    }
    html.push_str("<h2>Needs a look</h2>\n<ul class=\"list\">\n");
    for image in flagged {
        let name = escape(&image.image);
        html.push_str(&format!(
            "<li><a class=\"name\" href=\"#{name}\">{name}</a> {}</li>\n",
            badge(&image.status)
        ));
    }
    html.push_str("</ul>\n");
}

fn push_area(html: &mut String, area: &AreaReport) {
    html.push_str(&format!(
        "<section class=\"area\">\n<h2>{}</h2>\n",
        escape(area.title)
    ));
    for screen_images in area
        .images
        .chunk_by(|first, second| first.screen == second.screen)
    {
        push_screen(html, screen_images);
    }
    html.push_str("</section>\n");
}

/// One screen: its name and description, then its images (one per theme) side by side.
fn push_screen(html: &mut String, images: &[ImageReport]) {
    let Some(first) = images.first() else {
        return;
    };
    html.push_str(&format!(
        "<article class=\"screen\">\n<h3 class=\"name\">{}</h3>\n<p class=\"description\">{}</p>\n<div class=\"themes\">\n",
        escape(first.screen),
        escape(first.description)
    ));
    for image in images {
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
        badge(&image.status)
    ));
    match &image.status {
        ImageStatus::Matches => push_panel(html, None, ACTUAL_FOLDER, name),
        ImageStatus::Differs(comparison) => {
            push_comparison(html, comparison, ["Reference", "Actual"], name);
        }
        ImageStatus::Replaced(comparison) => {
            push_comparison(html, comparison, ["Old reference", "New reference"], name);
        }
        ImageStatus::Missing => {
            push_outcome(
                html,
                "No reference yet: an update run approves this screenshot as it is.",
            );
            push_panel(html, None, ACTUAL_FOLDER, name);
        }
        ImageStatus::Created => {
            push_outcome(html, "New reference, written by this update run.");
            push_panel(html, None, ACTUAL_FOLDER, name);
        }
        // The outputs were cleared before staging, and what is on disk now may not belong to
        // this run's result, so an error shows its message only.
        ImageStatus::Error(message) => {
            html.push_str(&format!("<pre class=\"error\">{}</pre>\n", escape(message)));
        }
    }
    html.push_str("</figure>\n");
}

/// The reference as it was, the screenshot of this run and the diff, each captioned.
fn push_comparison(
    html: &mut String,
    comparison: &PixelComparison,
    [reference_caption, actual_caption]: [&str; 2],
    image: &str,
) {
    push_outcome(html, &comparison.to_string());
    push_panel(html, Some(reference_caption), REFERENCE_COPY_FOLDER, image);
    push_panel(html, Some(actual_caption), ACTUAL_FOLDER, image);
    push_panel(
        html,
        Some("Diff: reference, actual, changes in magenta"),
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

fn badge(status: &ImageStatus) -> String {
    let label = match status {
        ImageStatus::Matches => "matches",
        ImageStatus::Differs(_) => "differs",
        ImageStatus::Missing => "missing",
        ImageStatus::Created => "new",
        ImageStatus::Replaced(_) => "updated",
        ImageStatus::Error(_) => "error",
    };
    format!("<span class=\"badge badge-{label}\">{label}</span>")
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
