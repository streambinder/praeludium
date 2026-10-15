//! Audio takes of pieces: storage, sharing and playback helpers.

use crate::db::{App, Result};
use crate::i18n::decimal;
use crate::render::date_dmy;
use rusqlite::params;
use serde::Serialize;

/// One audio take of one piece. Takes may also belong to a mock
/// audition item; the anchor is always the piece, and a piece holds
/// any number of recordings.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Recording {
    #[serde(rename = "ID")]
    pub id: i64,
    #[serde(rename = "PieceID")]
    pub piece_id: i64,
    #[serde(rename = "MockItemID", serialize_with = "crate::db::ser_null_i64")]
    pub mock_item_id: Option<i64>,
    #[serde(rename = "TakenOn")]
    pub taken_on: String, // YYYY-MM-DD
    /// Base name inside the recordings directory.
    #[serde(rename = "Filename")]
    pub filename: String,
    #[serde(rename = "DurationSec")]
    pub duration_sec: i64,
    #[serde(rename = "SizeBytes")]
    pub size_bytes: i64,
    /// Empty when the recording is not shared.
    #[serde(rename = "ShareToken")]
    pub share_token: String,
    #[serde(rename = "CreatedAt")]
    pub created_at: String,
}

/// One recording as the piece page renders it.
#[derive(Debug, Clone, Default, Serialize)]
pub struct RecordingRow {
    #[serde(rename = "Rec")]
    pub rec: Recording,
    #[serde(rename = "DateLabel")]
    pub date_label: String,
    #[serde(rename = "DurationLabel")]
    pub duration_label: String,
}

pub fn recording_rows(recs: &[Recording]) -> Vec<RecordingRow> {
    recs.iter()
        .map(|rec| RecordingRow {
            rec: rec.clone(),
            date_label: date_dmy(&rec.taken_on),
            duration_label: format!("{}:{:02}", rec.duration_sec / 60, rec.duration_sec % 60),
        })
        .collect()
}

pub const EXT_WEBM: &str = ".webm";

pub fn recording_ext(content_type: &str) -> &'static str {
    match () {
        _ if content_type.contains("webm") => EXT_WEBM,
        _ if content_type.contains("ogg") => ".ogg",
        _ if content_type.contains("mp4") => ".mp4",
        _ if content_type.contains("mpeg") => ".mp3",
        _ if content_type.contains("wav") => ".wav",
        _ => EXT_WEBM,
    }
}

pub fn recording_content_type(filename: &str) -> &'static str {
    match filename.rsplit('.').next() {
        Some("webm") => "audio/webm",
        Some("ogg") => "audio/ogg",
        Some("mp4") => "audio/mp4",
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        _ => "application/octet-stream",
    }
}

/// A random lowercase hex string of 2*bytes characters, from the
/// operating system entropy source.
pub fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    rand::fill(&mut buf[..]);
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// Render a byte count with a localized decimal.
pub fn human_bytes(lang: &str, n: i64) -> String {
    if n >= 1 << 20 {
        format!("{} MB", decimal(lang, n as f64 / (1 << 20) as f64))
    } else if n >= 1 << 10 {
        format!("{} KB", decimal(lang, n as f64 / (1 << 10) as f64))
    } else {
        format!("{n} B")
    }
}

const REC_COLS: &str = "id, piece_id, mock_item_id, taken_on, filename, duration_sec, size_bytes, COALESCE(share_token, ''), created_at";

fn scan_recording(row: &rusqlite::Row) -> Result<Recording> {
    Ok(Recording {
        id: row.get(0)?,
        piece_id: row.get(1)?,
        mock_item_id: row.get(2)?,
        taken_on: row.get(3)?,
        filename: row.get(4)?,
        duration_sec: row.get(5)?,
        size_bytes: row.get(6)?,
        share_token: row.get(7)?,
        created_at: row.get(8)?,
    })
}

impl App {
    pub fn recordings_dir(&self) -> std::io::Result<std::path::PathBuf> {
        let dir = self.data_dir.join("recordings");
        std::fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    pub fn piece_recordings(&self, piece_id: i64) -> Result<Vec<Recording>> {
        let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = db.prepare(&format!(
            "SELECT {REC_COLS} FROM recordings WHERE piece_id=? ORDER BY taken_on DESC, id DESC"
        ))?;
        let rows = stmt
            .query_map(params![piece_id], scan_recording)?
            .collect::<Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn recording_by_id(&self, id: i64) -> Result<Recording> {
        let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        db.query_row(
            &format!("SELECT {REC_COLS} FROM recordings WHERE id=?"),
            params![id],
            scan_recording,
        )
    }

    pub fn recording_by_token(&self, token: &str) -> Result<Recording> {
        let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        db.query_row(
            &format!("SELECT {REC_COLS} FROM recordings WHERE share_token=?"),
            params![token],
            scan_recording,
        )
    }

    /// The recordings taken during one mock, grouped-ready (flat
    /// list ordered by id).
    pub fn mock_recordings(&self, mock_id: i64) -> Result<Vec<Recording>> {
        let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = db.prepare(&format!(
            "SELECT {REC_COLS} FROM recordings WHERE mock_item_id IN (SELECT id FROM mock_items WHERE mock_id=?) ORDER BY id"
        ))?;
        let rows = stmt
            .query_map(params![mock_id], scan_recording)?
            .collect::<Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Total the stored takes for the settings page.
    pub fn recordings_usage(&self) -> Result<(i64, i64)> {
        let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        db.query_row(
            "SELECT COUNT(*), COALESCE(SUM(size_bytes), 0) FROM recordings",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
    }
}

/// The public player page behind a share token. The token is the
/// credential; revoking it closes the page.
pub fn share_page_html(title: &str, date: &str, token: &str) -> String {
    let esc = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    };
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{}</title>
<style>
body{{font-family:Georgia,serif;background:#faf6ee;color:#2c2418;margin:0;display:flex;min-height:100vh;align-items:center;justify-content:center}}
main{{background:#fffdf8;border:1px solid #e5dcc4;border-radius:14px;padding:1.6rem;max-width:30rem;width:calc(100% - 3rem);box-shadow:0 8px 30px rgba(90,64,25,.08)}}
h1{{font-size:1.25rem;margin:.2rem 0 .4rem}}
p{{color:#8a7a5c;margin:.2rem 0 1rem}}
audio{{width:100%}}
</style>
</head>
<body>
<main>
<p>Præludium</p>
<h1>{}</h1>
<p>{}</p>
<audio controls preload="metadata" src="/share/{}/audio"></audio>
</main>
</body>
</html>"#,
        esc(title),
        esc(title),
        esc(date),
        esc(token)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_and_content_types() {
        assert_eq!(recording_ext("audio/webm;codecs=opus"), ".webm");
        assert_eq!(recording_ext("audio/ogg"), ".ogg");
        assert_eq!(recording_ext("audio/mp4"), ".mp4");
        assert_eq!(recording_ext("audio/mpeg"), ".mp3");
        assert_eq!(recording_ext("audio/wav"), ".wav");
        assert_eq!(recording_ext("audio/x-unknown"), ".webm");
        assert_eq!(recording_content_type("a.webm"), "audio/webm");
        assert_eq!(recording_content_type("a.bin"), "application/octet-stream");
    }

    #[test]
    fn human_bytes_localizes() {
        assert_eq!(human_bytes("en", 512), "512 B");
        assert_eq!(human_bytes("en", 2048), "2.0 KB");
        assert_eq!(human_bytes("it", 3 * 1024 * 1024), "3,0 MB");
    }

    #[test]
    fn random_hex_has_expected_shape() {
        let s = random_hex(12);
        assert_eq!(s.len(), 24);
        assert!(s.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn rows_carry_labels() {
        let recs = vec![Recording {
            taken_on: "2026-10-14".into(),
            duration_sec: 75,
            ..Default::default()
        }];
        let rows = recording_rows(&recs);
        assert_eq!(rows[0].date_label, "14/10/2026");
        assert_eq!(rows[0].duration_label, "1:15");
    }
}
