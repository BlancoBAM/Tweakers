/// cosmic_themes.rs — Fetch, display and install COSMIC desktop themes
///
/// Scrapes cosmic-themes.org for the theme listing, parses each theme's
/// name / author / accent colour / download count, then can download a
/// theme tarball and install it to ~/.local/share/themes/cosmic/<name>/.

use std::path::PathBuf;
use serde::{Deserialize, Serialize};

// ─────────────────────────────── Theme model ──────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CosmicTheme {
    pub id: u32,
    pub name: String,
    pub author: String,
    pub homepage: String,
    pub downloads: u32,
    pub is_dark: bool,
    // Accent colour (parsed from --accent-color CSS var)
    pub accent_r: u8,
    pub accent_g: u8,
    pub accent_b: u8,
    // Background colour (parsed from --bg-color CSS var)
    pub bg_r: u8,
    pub bg_g: u8,
    pub bg_b: u8,
}

// ─────────────────────────────── Cache ────────────────────────────────────

fn cache_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|d| d.join("tweakers").join("cosmic_themes.json"))
}

fn read_cache() -> Option<(Vec<CosmicTheme>, std::time::SystemTime)> {
    let path = cache_path()?;
    let meta = std::fs::metadata(&path).ok()?;
    let modified = meta.modified().ok()?;
    let data = std::fs::read_to_string(&path).ok()?;
    let themes: Vec<CosmicTheme> = serde_json::from_str(&data).ok()?;
    Some((themes, modified))
}

fn write_cache(themes: &[CosmicTheme]) {
    if let Some(path) = cache_path() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(themes) {
            let _ = std::fs::write(path, json);
        }
    }
}

const CACHE_TTL_SECS: u64 = 86_400; // 24 hours

// ─────────────────────────────── Fetch & parse ────────────────────────────

/// Fetch themes from cosmic-themes.org.
/// Returns cached results if they are less than 24 h old.
pub async fn fetch_themes() -> Result<Vec<CosmicTheme>, Box<dyn std::error::Error + Send + Sync>> {
    // Check cache first
    if let Some((themes, modified)) = read_cache() {
        if let Ok(age) = modified.elapsed() {
            if age.as_secs() < CACHE_TTL_SECS && !themes.is_empty() {
                log::info!("cosmic_themes: serving {} themes from cache", themes.len());
                return Ok(themes);
            }
        }
    }

    log::info!("cosmic_themes: fetching from cosmic-themes.org …");

    // Tokio blocking task for reqwest blocking client
    let themes = tokio::task::spawn_blocking(|| -> Result<Vec<CosmicTheme>, Box<dyn std::error::Error + Send + Sync>> {
        let client = reqwest::blocking::Client::builder()
            .user_agent("Tweakers/1.0 (Lilith Linux)")
            .timeout(std::time::Duration::from_secs(15))
            .build()?;

        let html = client.get("https://cosmic-themes.org/").send()?.text()?;
        parse_themes_html(&html)
    })
    .await??;

    write_cache(&themes);
    log::info!("cosmic_themes: fetched {} themes", themes.len());
    Ok(themes)
}

/// Parse theme metadata from the cosmic-themes.org HTML response.
fn parse_themes_html(html: &str) -> Result<Vec<CosmicTheme>, Box<dyn std::error::Error + Send + Sync>> {
    use scraper::{Html, Selector};

    let document = Html::parse_document(html);
    let mut themes = Vec::new();

    // Each theme is in a <section> containing a <div id="theme-preview-{id}">
    let section_sel = Selector::parse("section").unwrap();
    let preview_sel = Selector::parse("[id^='theme-preview-']").unwrap();
    let style_sel = Selector::parse("style").unwrap();
    let h3_sel = Selector::parse("h3").unwrap();

    for section in document.select(&section_sel) {
        // Find the preview div to get the ID
        let preview_div = match section.select(&preview_sel).next() {
            Some(d) => d,
            None => continue,
        };

        let preview_id = preview_div.value().attr("id").unwrap_or("");
        let id: u32 = preview_id
            .strip_prefix("theme-preview-")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        if id == 0 {
            continue;
        }

        // Theme name from <h3>
        let name = section
            .select(&h3_sel)
            .next()
            .map(|el| el.inner_html().trim().to_string())
            .unwrap_or_default();

        // Author, homepage, downloads from .theme-info rows
        let mut author = String::new();
        let mut homepage = String::new();
        let mut downloads: u32 = 0;

        // Parse the inline HTML text for theme-info divs
        let inner = section.html();
        for line in inner.lines() {
            let line = line.trim();
            if line.contains("Author") && author.is_empty() {
                // Next significant text value after "Author" label
                if let Some(val) = extract_next_text_value(&inner, "Author") {
                    author = val;
                }
            }
            if line.contains("Downloads") && downloads == 0 {
                if let Some(val) = extract_next_text_value(&inner, "Downloads") {
                    downloads = val.trim().parse().unwrap_or(0);
                }
            }
        }

        // Homepage from <a href> in theme-info
        let link_sel = Selector::parse("a[href^='http']").unwrap();
        if let Some(link) = section.select(&link_sel).next() {
            homepage = link.value().attr("href").unwrap_or("").to_string();
        }

        // CSS variables from <style> tag
        let style_text = section
            .select(&style_sel)
            .next()
            .map(|s| s.inner_html())
            .unwrap_or_default();

        let (accent_r, accent_g, accent_b) = parse_css_rgba(&style_text, "--accent-color");
        let (bg_r, bg_g, bg_b) = parse_css_rgba(&style_text, "--bg-color");
        let is_dark = style_text.contains("--is-dark: 1");

        if name.is_empty() {
            continue;
        }

        themes.push(CosmicTheme {
            id,
            name,
            author,
            homepage,
            downloads,
            is_dark,
            accent_r,
            accent_g,
            accent_b,
            bg_r,
            bg_g,
            bg_b,
        });
    }

    // Sort by downloads descending (popular first)
    themes.sort_by(|a, b| b.downloads.cmp(&a.downloads));
    Ok(themes)
}

/// Extract the text content that follows a label in the theme-info HTML.
fn extract_next_text_value(html: &str, label: &str) -> Option<String> {
    // Look for the label then grab the next text-end div's content
    let idx = html.find(label)?;
    let after = &html[idx..];
    // Find the next text-end div and extract text between > and <
    let te = after.find("text-end")?;
    let after_te = &after[te..];
    let gt = after_te.find('>')?;
    let lt = after_te[gt + 1..].find('<')?;
    let value = after_te[gt + 1..gt + 1 + lt].trim().to_string();
    // Strip HTML tags if any
    if value.contains('<') {
        return None;
    }
    Some(value)
}

/// Parse an rgba() value from a CSS custom property declaration.
/// Returns (r, g, b) as u8, defaulting to (100, 100, 255) on failure.
fn parse_css_rgba(css: &str, var_name: &str) -> (u8, u8, u8) {
    // Match "--accent-color: rgba(R, G, B, A);"
    let search = format!("{}: rgba(", var_name);
    if let Some(start) = css.find(&search) {
        let after = &css[start + search.len()..];
        if let Some(end) = after.find(')') {
            let parts: Vec<&str> = after[..end].split(',').collect();
            if parts.len() >= 3 {
                let r = parts[0].trim().parse::<f32>().unwrap_or(100.0) as u8;
                let g = parts[1].trim().parse::<f32>().unwrap_or(100.0) as u8;
                let b = parts[2].trim().parse::<f32>().unwrap_or(255.0) as u8;
                return (r, g, b);
            }
        }
    }
    (100, 100, 255)
}

// ─────────────────────────────── Download & Install ───────────────────────

/// Download a theme tarball from cosmic-themes.org and install it to
/// ~/.local/share/themes/cosmic/<theme-name>/
pub async fn download_and_install_theme(
    theme: &CosmicTheme,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let theme_id = theme.id;
    let theme_name = theme.name.clone();

    log::info!("cosmic_themes: downloading theme '{}' (id={})", theme_name, theme_id);

    // Step 1: Fetch the theme page to grab the CSRF token
    let (csrf_token, zip_bytes) = tokio::task::spawn_blocking(move || {
        fetch_and_download(theme_id)
    })
    .await??;

    let _ = csrf_token; // used inside fetch_and_download

    // Step 2: Determine install directory
    let install_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("themes")
        .join("cosmic")
        .join(&theme_name);

    tokio::fs::create_dir_all(&install_dir).await?;

    // Step 3: Extract zip/tar to install_dir
    let install_dir_clone = install_dir.clone();
    tokio::task::spawn_blocking(move || {
        extract_theme_archive(&zip_bytes, &install_dir_clone)
    })
    .await??;

    log::info!(
        "cosmic_themes: installed '{}' to {}",
        theme_name,
        install_dir.display()
    );

    Ok(())
}

/// Synchronously fetch the download page for CSRF then POST to download the archive.
fn fetch_and_download(theme_id: u32) -> Result<(String, Vec<u8>), Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("Tweakers/1.0 (Lilith Linux)")
        .timeout(std::time::Duration::from_secs(30))
        .cookie_store(true)
        .build()?;

    // GET the individual theme page to capture CSRF token
    let page_url = format!("https://cosmic-themes.org/{}/", theme_id);
    let page_resp = client.get(&page_url).send()?;
    let page_html = page_resp.text()?;

    let csrf = extract_csrf(&page_html)
        .ok_or("Could not find CSRF token on theme page")?;

    // POST to the download endpoint
    let download_url = format!("https://cosmic-themes.org/{}/download/", theme_id);
    let bytes = client
        .post(&download_url)
        .header("Referer", &page_url)
        .form(&[("csrfmiddlewaretoken", &csrf)])
        .send()?
        .bytes()?
        .to_vec();

    Ok((csrf, bytes))
}

fn extract_csrf(html: &str) -> Option<String> {
    // <input type="hidden" name="csrfmiddlewaretoken" value="...">
    let needle = "csrfmiddlewaretoken\" value=\"";
    let start = html.find(needle)? + needle.len();
    let end = html[start..].find('"')?;
    Some(html[start..start + end].to_string())
}

/// Extract a zip or tar.gz archive to the destination directory.
fn extract_theme_archive(
    bytes: &[u8],
    dest: &PathBuf,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::io::Cursor;

    // Try zip first
    if bytes.starts_with(b"PK") {
        let cursor = Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor)?;
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let outpath = dest.join(sanitize_zip_path(file.name()));
            if file.is_dir() {
                std::fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent() {
                    std::fs::create_dir_all(p)?;
                }
                let mut out = std::fs::File::create(&outpath)?;
                std::io::copy(&mut file, &mut out)?;
            }
        }
        return Ok(());
    }

    // Try tar.gz
    let cursor = Cursor::new(bytes);
    let gz = flate2::read::GzDecoder::new(cursor);
    let mut archive = tar::Archive::new(gz);
    archive.unpack(dest)?;

    Ok(())
}

fn sanitize_zip_path(name: &str) -> &str {
    // Strip leading ../ or / to avoid path traversal
    let stripped = name.trim_start_matches('/');
    if let Some(pos) = stripped.rfind('/') {
        &stripped[pos + 1..]
    } else {
        stripped
    }
}
