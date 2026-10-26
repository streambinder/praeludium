//! Database access: schema, migrations and queries.
//!
//! The connection sits behind a mutex, which mirrors the single
//! connection pool of the previous implementation: queries are
//! collected fully before any follow-up query runs.

use crate::score::Coeffs;
use rusqlite::{Connection, OptionalExtension, params};
use std::path::PathBuf;
use std::sync::Mutex;

pub type Result<T> = rusqlite::Result<T>;

/// Serialize an optional integer the way Go templates read a
/// sql.NullInt64: an object with Valid and Int64 fields.
pub fn ser_null_i64<S: serde::Serializer>(
    v: &Option<i64>,
    s: S,
) -> std::result::Result<S::Ok, S::Error> {
    #[derive(serde::Serialize)]
    struct NullI64 {
        #[serde(rename = "Valid")]
        valid: bool,
        #[serde(rename = "Int64")]
        int64: i64,
    }
    let value = NullI64 {
        valid: v.is_some(),
        int64: v.unwrap_or(0),
    };
    <NullI64 as serde::Serialize>::serialize(&value, s)
}

/// The application state shared by every handler.
pub struct App {
    pub db: Mutex<Connection>,
    pub data_dir: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct Audition {
    #[serde(rename = "ID")]
    pub id: i64,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Date")]
    pub date: String, // YYYY-MM-DD
    #[serde(rename = "Weight")]
    pub weight: i64,
    #[serde(rename = "Archived")]
    pub archived: bool,
    /// Per-link excerpt, set only when read through piece_audition.
    #[serde(rename = "Excerpt")]
    pub excerpt: String,
    /// Per-link round, set only when read through piece_audition.
    #[serde(rename = "RoundID", serialize_with = "ser_null_i64")]
    pub round_id: Option<i64>,
    /// Per-link round name, set only when read through piece_audition.
    #[serde(rename = "RoundName")]
    pub round_name: String,
    /// Per-link segment labels, set only when read through
    /// piece_audition; empty when the link asks the whole piece.
    #[serde(rename = "SegmentLabels")]
    pub segment_labels: Vec<String>,
    /// Mean preparation level 0..4 of its pieces, stamped on demand.
    #[serde(rename = "Level")]
    pub level: i64,
    /// Mean preparation score 0..1 of its pieces, stamped on demand.
    #[serde(rename = "Prep")]
    pub prep: f64,
}

/// Piece kinds.
pub const KIND_EXCERPT: &str = "excerpt";
pub const KIND_SOLO: &str = "solo";

/// Session note markers for days without practice.
pub const NOTE_SKIPPED: &str = "skipped";
pub const NOTE_POSTPONED: &str = "postponed";
pub const NOTE_AUTO_SKIPPED: &str = "auto-skipped";

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct Piece {
    #[serde(rename = "ID")]
    pub id: i64,
    #[serde(rename = "Composer")]
    pub composer: String,
    #[serde(rename = "Work")]
    pub work: String,
    #[serde(rename = "Movement")]
    pub movement: String,
    #[serde(rename = "Kind")]
    pub kind: String, // 'excerpt' | 'solo'
    /// 1..5, user-calibrated; drives the prep-map tile size.
    #[serde(rename = "Difficulty")]
    pub difficulty: i64,
    /// Optional target tempo; None means no tempo signal.
    #[serde(rename = "TargetBPM", serialize_with = "ser_null_i64")]
    pub target_bpm: Option<i64>,
    #[serde(rename = "Archived")]
    pub archived: bool,
    #[serde(rename = "Auditions")]
    pub auditions: Vec<Audition>,
    /// At least one non-archived audition with date >= today.
    #[serde(rename = "HasActive")]
    pub has_active: bool,
    /// Preparation level 0..4, stamped on demand (not persisted).
    #[serde(rename = "Level")]
    pub level: i64,
    /// Preparation score 0..1, stamped on demand (not persisted).
    #[serde(rename = "Prep")]
    pub prep: f64,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct Session {
    #[serde(rename = "ID")]
    pub id: i64,
    #[serde(rename = "Date")]
    pub date: String,
    #[serde(rename = "PieceID")]
    pub piece_id: i64,
    #[serde(rename = "Minutes")]
    pub minutes: i64,
    #[serde(rename = "Confidence")]
    pub confidence: i64,
    #[serde(rename = "Note")]
    pub note: String,
    /// Optional metronome mark; None when unrecorded.
    #[serde(rename = "BPM", serialize_with = "ser_null_i64")]
    pub bpm: Option<i64>,
    /// Segment the session covered; None means the whole piece.
    #[serde(rename = "SegmentID", serialize_with = "ser_null_i64")]
    pub segment_id: Option<i64>,
}

/// A named or ranged part of a piece. A segment is one contiguous
/// range between two endpoints; an endpoint is an anchor kind
/// (`start`, `end`, `bar`, `mark`), an anchor value, and a signed
/// offset in bars.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct Segment {
    #[serde(rename = "ID")]
    pub id: i64,
    #[serde(rename = "PieceID")]
    pub piece_id: i64,
    #[serde(rename = "Alias")]
    pub alias: String,
    #[serde(rename = "StartKind")]
    pub start_kind: String,
    #[serde(rename = "StartValue")]
    pub start_value: String,
    #[serde(rename = "StartOffset")]
    pub start_offset: i64,
    #[serde(rename = "EndKind")]
    pub end_kind: String,
    #[serde(rename = "EndValue")]
    pub end_value: String,
    #[serde(rename = "EndOffset")]
    pub end_offset: i64,
    /// Display label (alias or rendered range), stamped on read.
    #[serde(rename = "Label")]
    pub label: String,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct Availability {
    #[serde(rename = "ID")]
    pub id: i64,
    #[serde(rename = "Date")]
    pub date: String,
    #[serde(rename = "StartMin")]
    pub start_min: i64,
    #[serde(rename = "EndMin")]
    pub end_min: i64,
    #[serde(rename = "Label")]
    pub label: String,
    #[serde(rename = "Kind")]
    pub kind: String, // 'free' | 'busy'
}

/// One round of an audition (preliminary, semifinal, final). A piece
/// link may name a round; no round means the piece is asked in every
/// round.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct Round {
    #[serde(rename = "ID")]
    pub id: i64,
    #[serde(rename = "AuditionID")]
    pub audition_id: i64,
    #[serde(rename = "Position")]
    pub position: i64,
    #[serde(rename = "Name")]
    pub name: String,
}

/// One external resource attached to an audition (official notice,
/// orchestral parts PDF, ...).
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct AuditionLink {
    #[serde(rename = "ID")]
    pub id: i64,
    #[serde(rename = "AuditionID")]
    pub audition_id: i64,
    #[serde(rename = "Label")]
    pub label: String,
    #[serde(rename = "URL")]
    pub url: String,
}

pub fn open_db(path: &std::path::Path) -> Result<Connection> {
    let db = Connection::open(path)?;
    db.pragma_update(None, "journal_mode", "WAL")?;
    db.pragma_update(None, "foreign_keys", "ON")?;
    migrate(&db)?;
    Ok(db)
}

/// In-memory database for tests.
pub fn open_memory() -> Result<Connection> {
    let db = Connection::open_in_memory()?;
    db.pragma_update(None, "foreign_keys", "ON")?;
    migrate(&db)?;
    Ok(db)
}

/// Add one column to a table. A duplicate-column error means the
/// migration already ran and is not a failure.
fn add_column(db: &Connection, stmt: &str, label: &str) -> Result<()> {
    match db.execute_batch(stmt) {
        Ok(()) => Ok(()),
        Err(err) => {
            if err.to_string().contains("duplicate column name") {
                Ok(())
            } else {
                eprintln!("migrate {label}: {err}");
                Err(err)
            }
        }
    }
}

fn migrate(db: &Connection) -> Result<()> {
    rename_legacy_schema(db)?;
    let stmts = [
        "CREATE TABLE IF NOT EXISTS auditions(
			id INTEGER PRIMARY KEY,
			name TEXT NOT NULL,
			city TEXT NOT NULL DEFAULT '',
			audition_date TEXT NOT NULL,
			weight INTEGER NOT NULL DEFAULT 1,
			archived_at TEXT,
			created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
		)",
        "CREATE TABLE IF NOT EXISTS pieces(
			id INTEGER PRIMARY KEY,
			composer TEXT NOT NULL,
			work TEXT NOT NULL,
			movement TEXT NOT NULL DEFAULT '',
			excerpt TEXT NOT NULL DEFAULT '',
			kind TEXT NOT NULL DEFAULT 'excerpt',
			archived_at TEXT,
			created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
		)",
        "CREATE TABLE IF NOT EXISTS piece_audition(
			piece_id INTEGER NOT NULL REFERENCES pieces(id) ON DELETE CASCADE,
			audition_id INTEGER NOT NULL REFERENCES auditions(id) ON DELETE CASCADE,
			PRIMARY KEY(piece_id, audition_id)
		)",
        "CREATE TABLE IF NOT EXISTS audition_links(
			id INTEGER PRIMARY KEY,
			audition_id INTEGER NOT NULL REFERENCES auditions(id) ON DELETE CASCADE,
			label TEXT NOT NULL DEFAULT '',
			url TEXT NOT NULL
		)",
        "CREATE TABLE IF NOT EXISTS sessions(
			id INTEGER PRIMARY KEY,
			date TEXT NOT NULL,
			piece_id INTEGER NOT NULL REFERENCES pieces(id) ON DELETE CASCADE,
			minutes INTEGER NOT NULL DEFAULT 0,
			confidence INTEGER NOT NULL DEFAULT 0,
			note TEXT NOT NULL DEFAULT '',
			created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
		)",
        "CREATE INDEX IF NOT EXISTS idx_sessions_piece_date ON sessions(piece_id, date)",
        "CREATE TABLE IF NOT EXISTS availability(
			id INTEGER PRIMARY KEY,
			date TEXT NOT NULL,
			start_min INTEGER NOT NULL,
			end_min INTEGER NOT NULL,
			label TEXT NOT NULL DEFAULT '',
			kind TEXT NOT NULL DEFAULT 'free',
			created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
		)",
        "CREATE INDEX IF NOT EXISTS idx_availability_date ON availability(date)",
        "CREATE TABLE IF NOT EXISTS settings(
			key TEXT PRIMARY KEY,
			value TEXT NOT NULL
		)",
        "CREATE TABLE IF NOT EXISTS recordings(
			id INTEGER PRIMARY KEY,
			piece_id INTEGER NOT NULL REFERENCES pieces(id) ON DELETE CASCADE,
			mock_item_id INTEGER,
			taken_on TEXT NOT NULL,
			filename TEXT NOT NULL,
			duration_sec INTEGER NOT NULL DEFAULT 0,
			size_bytes INTEGER NOT NULL DEFAULT 0,
			share_token TEXT,
			created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
		)",
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_recordings_share_token ON recordings(share_token) WHERE share_token IS NOT NULL",
        "CREATE TABLE IF NOT EXISTS mocks(
			id INTEGER PRIMARY KEY,
			audition_id INTEGER NOT NULL REFERENCES auditions(id) ON DELETE CASCADE,
			round_id INTEGER REFERENCES audition_rounds(id) ON DELETE SET NULL,
			date TEXT NOT NULL,
			record_take INTEGER NOT NULL DEFAULT 1,
			created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
		)",
        "CREATE TABLE IF NOT EXISTS mock_items(
			id INTEGER PRIMARY KEY,
			mock_id INTEGER NOT NULL REFERENCES mocks(id) ON DELETE CASCADE,
			piece_id INTEGER NOT NULL REFERENCES pieces(id) ON DELETE CASCADE,
			position INTEGER NOT NULL,
			confidence INTEGER NOT NULL DEFAULT 0,
			note TEXT NOT NULL DEFAULT ''
		)",
        "CREATE TABLE IF NOT EXISTS audition_rounds(
			id INTEGER PRIMARY KEY,
			audition_id INTEGER NOT NULL REFERENCES auditions(id) ON DELETE CASCADE,
			position INTEGER NOT NULL DEFAULT 1,
			name TEXT NOT NULL DEFAULT ''
		)",
        "CREATE TABLE IF NOT EXISTS segments(
			id INTEGER PRIMARY KEY,
			piece_id INTEGER NOT NULL REFERENCES pieces(id) ON DELETE CASCADE,
			alias TEXT NOT NULL DEFAULT '',
			start_kind TEXT NOT NULL DEFAULT 'start',
			start_value TEXT NOT NULL DEFAULT '',
			start_offset INTEGER NOT NULL DEFAULT 0,
			end_kind TEXT NOT NULL DEFAULT 'end',
			end_value TEXT NOT NULL DEFAULT '',
			end_offset INTEGER NOT NULL DEFAULT 0
		)",
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_segments_alias ON segments(piece_id, alias) WHERE alias <> ''",
        "CREATE TABLE IF NOT EXISTS link_segments(
			piece_id INTEGER NOT NULL REFERENCES pieces(id) ON DELETE CASCADE,
			audition_id INTEGER NOT NULL REFERENCES auditions(id) ON DELETE CASCADE,
			segment_id INTEGER NOT NULL REFERENCES segments(id) ON DELETE CASCADE,
			PRIMARY KEY(piece_id, audition_id, segment_id)
		)",
    ];
    for s in stmts {
        db.execute_batch(s)?;
    }
    // auditions.archived_at was added after the first schema revision.
    add_column(
        db,
        "ALTER TABLE auditions ADD COLUMN archived_at TEXT",
        "archived_at",
    )?;
    // pieces.difficulty was added after the first schema revision.
    add_column(
        db,
        "ALTER TABLE pieces ADD COLUMN difficulty INTEGER NOT NULL DEFAULT 3",
        "difficulty",
    )?;
    // piece_audition.excerpt holds the excerpt each audition asks
    // for; the column started out as notes and is renamed when present.
    ensure_link_excerpt(db)?;
    // piece_audition.round_id names the round a linked piece belongs
    // to; NULL means the piece is asked in every round.
    add_column(
        db,
        "ALTER TABLE piece_audition ADD COLUMN round_id INTEGER REFERENCES audition_rounds(id) ON DELETE SET NULL",
        "round_id",
    )?;
    // pieces.target_bpm is the optional target tempo of a piece; NULL
    // means the piece carries no tempo signal.
    add_column(
        db,
        "ALTER TABLE pieces ADD COLUMN target_bpm INTEGER",
        "target_bpm",
    )?;
    // sessions.bpm holds the optional metronome mark of a practice
    // session; NULL when the player did not record one.
    add_column(db, "ALTER TABLE sessions ADD COLUMN bpm INTEGER", "bpm")?;
    // sessions.segment_id names the segment a session covered;
    // NULL means the session covered the whole piece.
    add_column(
        db,
        "ALTER TABLE sessions ADD COLUMN segment_id INTEGER REFERENCES segments(id) ON DELETE SET NULL",
        "segment_id",
    )?;
    // Stored values started out in Italian; rewrite them in English.
    for q in [
        "UPDATE sessions SET note='skipped' WHERE note='saltato'",
        "UPDATE sessions SET note='auto-skipped' WHERE note='auto-saltato'",
        "UPDATE sessions SET note='postponed' WHERE note='rimandato'",
        "UPDATE pieces SET kind='excerpt' WHERE kind='passo'",
    ] {
        db.execute_batch(q)?;
    }
    Ok(())
}

/// Rename the original Italian tables and columns to English. Every
/// step is guarded, so the function is a no-op on databases that
/// already use the English schema. Stored values are rewritten at
/// the end of migrate, once the tables exist for sure.
fn rename_legacy_schema(db: &Connection) -> Result<()> {
    for (from, to) in [
        ("concorsi", "auditions"),
        ("piece_concorso", "piece_audition"),
        ("concorso_links", "audition_links"),
    ] {
        if table_exists(db, from)? {
            db.execute_batch(&format!("ALTER TABLE {from} RENAME TO {to}"))?;
        }
    }
    for (table, from, to) in [
        ("piece_audition", "concorso_id", "audition_id"),
        ("piece_audition", "estratto", "excerpt"),
        ("audition_links", "concorso_id", "audition_id"),
        ("sessions", "tempo", "bpm"),
    ] {
        if table_columns(db, table)?.contains(from) {
            db.execute_batch(&format!("ALTER TABLE {table} RENAME COLUMN {from} TO {to}"))?;
        }
    }
    Ok(())
}

fn table_exists(db: &Connection, name: &str) -> Result<bool> {
    let n: i64 = db.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?",
        params![name],
        |row| row.get(0),
    )?;
    Ok(n > 0)
}

/// Rename the original notes column to excerpt, or add excerpt on
/// installs that never had notes.
fn ensure_link_excerpt(db: &Connection) -> Result<()> {
    let cols = table_columns(db, "piece_audition")?;
    if cols.contains("excerpt") {
        return Ok(());
    }
    if cols.contains("notes") {
        db.execute_batch("ALTER TABLE piece_audition RENAME COLUMN notes TO excerpt")?;
    } else {
        db.execute_batch("ALTER TABLE piece_audition ADD COLUMN excerpt TEXT NOT NULL DEFAULT ''")?;
    }
    Ok(())
}

fn table_columns(db: &Connection, table: &str) -> Result<std::collections::HashSet<String>> {
    let mut stmt = db.prepare(&format!("PRAGMA table_info({table})"))?;
    let names = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<std::collections::HashSet<String>>>()?;
    Ok(names)
}

pub fn today_str() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// (b − a) in whole days for YYYY-MM-DD strings.
pub fn days_between(a: &str, b: &str) -> Result<i64> {
    let parse = |s: &str| {
        chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })
    };
    let ta = parse(a)?;
    let tb = parse(b)?;
    Ok((tb - ta).num_days())
}

fn scan_audition(row: &rusqlite::Row) -> Result<Audition> {
    Ok(Audition {
        id: row.get(0)?,
        name: row.get(1)?,
        date: row.get(2)?,
        weight: row.get(3)?,
        archived: row.get::<_, i64>(4)? != 0,
        ..Default::default()
    })
}

impl App {
    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.db.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn list_auditions(&self, include_archived: bool) -> Result<Vec<Audition>> {
        let mut q = String::from(
            "SELECT id, name, audition_date, weight, archived_at IS NOT NULL FROM auditions",
        );
        if !include_archived {
            q += " WHERE archived_at IS NULL";
        }
        q += " ORDER BY archived_at IS NOT NULL, audition_date";
        let db = self.conn();
        let mut stmt = db.prepare(&q)?;
        let rows = stmt
            .query_map([], scan_audition)?
            .collect::<Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn piece_auditions(&self, piece_id: i64) -> Result<Vec<Audition>> {
        let db = self.conn();
        let mut stmt = db.prepare(
            "SELECT c.id, c.name, c.audition_date, c.weight, c.archived_at IS NOT NULL, pc.excerpt,
			pc.round_id, COALESCE(rd.name, '')
			FROM auditions c JOIN piece_audition pc ON pc.audition_id=c.id
			LEFT JOIN audition_rounds rd ON rd.id = pc.round_id
			WHERE pc.piece_id=? ORDER BY c.audition_date",
        )?;
        let mut rows = stmt
            .query_map(params![piece_id], |row| {
                Ok(Audition {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    date: row.get(2)?,
                    weight: row.get(3)?,
                    archived: row.get::<_, i64>(4)? != 0,
                    excerpt: row.get(5)?,
                    round_id: row.get(6)?,
                    round_name: row.get(7)?,
                    ..Default::default()
                })
            })?
            .collect::<Result<Vec<_>>>()?;
        drop(stmt);
        drop(db);
        if !rows.is_empty() {
            let by_id: std::collections::HashMap<i64, String> = self
                .segments_for_piece(piece_id)?
                .into_iter()
                .map(|s| (s.id, s.label))
                .collect();
            for c in &mut rows {
                let ids = self.link_segment_ids(piece_id, c.id)?;
                c.segment_labels = ids.iter().filter_map(|id| by_id.get(id).cloned()).collect();
            }
        }
        Ok(rows)
    }

    pub fn audition_rounds(&self, audition_id: i64) -> Result<Vec<Round>> {
        let db = self.conn();
        let mut stmt = db.prepare(
            "SELECT id, audition_id, position, name FROM audition_rounds
			WHERE audition_id=? ORDER BY position, id",
        )?;
        let rows = stmt
            .query_map(params![audition_id], |row| {
                Ok(Round {
                    id: row.get(0)?,
                    audition_id: row.get(1)?,
                    position: row.get(2)?,
                    name: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn audition_links(&self, audition_id: i64) -> Result<Vec<AuditionLink>> {
        let db = self.conn();
        let mut stmt = db.prepare(
            "SELECT id, audition_id, label, url FROM audition_links
			WHERE audition_id=? ORDER BY id",
        )?;
        let rows = stmt
            .query_map(params![audition_id], |row| {
                Ok(AuditionLink {
                    id: row.get(0)?,
                    audition_id: row.get(1)?,
                    label: row.get(2)?,
                    url: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn add_audition_link(&self, audition_id: i64, label: &str, url: &str) -> Result<()> {
        self.conn().execute(
            "INSERT INTO audition_links(audition_id, label, url) VALUES(?,?,?)",
            params![audition_id, label, url],
        )?;
        Ok(())
    }

    pub fn delete_audition_link(&self, audition_id: i64, link_id: i64) -> Result<()> {
        self.conn().execute(
            "DELETE FROM audition_links WHERE id=? AND audition_id=?",
            params![link_id, audition_id],
        )?;
        Ok(())
    }

    /// Return the segments of a piece, oldest first, each with
    /// its display label stamped.
    pub fn segments_for_piece(&self, piece_id: i64) -> Result<Vec<Segment>> {
        let db = self.conn();
        let mut stmt = db.prepare(
            "SELECT id, piece_id, alias, start_kind, start_value, start_offset,
			end_kind, end_value, end_offset FROM segments
			WHERE piece_id=? ORDER BY id",
        )?;
        let mut rows = stmt
            .query_map(params![piece_id], |row| {
                Ok(Segment {
                    id: row.get(0)?,
                    piece_id: row.get(1)?,
                    alias: row.get(2)?,
                    start_kind: row.get(3)?,
                    start_value: row.get(4)?,
                    start_offset: row.get(5)?,
                    end_kind: row.get(6)?,
                    end_value: row.get(7)?,
                    end_offset: row.get(8)?,
                    label: String::new(),
                })
            })?
            .collect::<Result<Vec<_>>>()?;
        for seg in &mut rows {
            seg.label = crate::segments::segment_label(seg);
        }
        Ok(rows)
    }

    /// Insert a segment on a piece and return its id. A non-empty
    /// alias must be unique within the piece (unique index).
    pub fn add_segment(&self, seg: &Segment) -> Result<i64> {
        let db = self.conn();
        db.execute(
            "INSERT INTO segments(piece_id, alias, start_kind, start_value, start_offset,
			end_kind, end_value, end_offset) VALUES(?,?,?,?,?,?,?,?)",
            params![
                seg.piece_id,
                seg.alias,
                seg.start_kind,
                seg.start_value,
                seg.start_offset,
                seg.end_kind,
                seg.end_value,
                seg.end_offset
            ],
        )?;
        Ok(db.last_insert_rowid())
    }

    /// Count the links that reference a segment.
    pub fn segment_link_count(&self, segment_id: i64) -> Result<i64> {
        Ok(self.conn().query_row(
            "SELECT COUNT(*) FROM link_segments WHERE segment_id=?",
            params![segment_id],
            |row| row.get(0),
        )?)
    }

    /// Delete a segment. A segment that any link references is
    /// kept, and the function returns false.
    pub fn delete_segment(&self, segment_id: i64) -> Result<bool> {
        if self.segment_link_count(segment_id)? > 0 {
            return Ok(false);
        }
        self.conn()
            .execute("DELETE FROM segments WHERE id=?", params![segment_id])?;
        Ok(true)
    }

    /// Return the segment ids a piece-audition link references,
    /// in definition order.
    pub fn link_segment_ids(&self, piece_id: i64, audition_id: i64) -> Result<Vec<i64>> {
        let db = self.conn();
        let mut stmt = db.prepare(
            "SELECT segment_id FROM link_segments
			WHERE piece_id=? AND audition_id=? ORDER BY segment_id",
        )?;
        let rows = stmt
            .query_map(params![piece_id, audition_id], |row| row.get(0))?
            .collect::<Result<Vec<i64>>>()?;
        Ok(rows)
    }

    /// Remove every segment reference of a piece, across all its
    /// links. The piece form rebuild calls this before it writes
    /// the new sets.
    pub fn clear_piece_link_segments(&self, piece_id: i64) -> Result<()> {
        self.conn().execute(
            "DELETE FROM link_segments WHERE piece_id=?",
            params![piece_id],
        )?;
        Ok(())
    }

    /// Replace the segment set of a piece-audition link. An id
    /// that names no segment of the piece is ignored.
    pub fn set_link_segments(
        &self,
        piece_id: i64,
        audition_id: i64,
        segment_ids: &[i64],
    ) -> Result<()> {
        let db = self.conn();
        db.execute(
            "DELETE FROM link_segments WHERE piece_id=? AND audition_id=?",
            params![piece_id, audition_id],
        )?;
        for id in segment_ids {
            db.execute(
                "INSERT INTO link_segments(piece_id, audition_id, segment_id)
				SELECT ?,?, id FROM segments WHERE id=? AND piece_id=?",
                params![piece_id, audition_id, id, piece_id],
            )?;
        }
        Ok(())
    }

    /// Return the pieces matching the filters; query is a free-text
    /// search over every text field of the piece (composer, work,
    /// movement, kind) plus its audition names and excerpts.
    pub fn list_pieces(
        &self,
        audition_id: i64,
        kind: &str,
        include_archived: bool,
        query: &str,
    ) -> Result<Vec<Piece>> {
        let mut q = String::from(
            "SELECT DISTINCT p.id, p.composer, p.work, p.movement, p.kind, p.difficulty,
			p.archived_at IS NOT NULL, p.target_bpm FROM pieces p",
        );
        let mut args: Vec<rusqlite::types::Value> = Vec::new();
        let mut where_clause = String::new();
        if audition_id > 0 {
            q += " JOIN piece_audition pc ON pc.piece_id=p.id";
            where_clause += " AND pc.audition_id=?";
            args.push(rusqlite::types::Value::Integer(audition_id));
        }
        if kind == KIND_EXCERPT || kind == KIND_SOLO {
            where_clause += " AND p.kind=?";
            args.push(rusqlite::types::Value::Text(kind.to_string()));
        }
        if !include_archived {
            where_clause += " AND p.archived_at IS NULL";
        }
        let query = query.trim();
        if !query.is_empty() {
            let like = format!("%{}%", query.to_lowercase());
            where_clause += " AND (LOWER(p.composer) LIKE ? OR LOWER(p.work) LIKE ? OR LOWER(p.movement) LIKE ? OR LOWER(p.kind) LIKE ?
				OR EXISTS (SELECT 1 FROM piece_audition pcq JOIN auditions cq ON cq.id = pcq.audition_id
					WHERE pcq.piece_id = p.id AND (LOWER(cq.name) LIKE ? OR LOWER(pcq.excerpt) LIKE ?)))";
            for _ in 0..6 {
                args.push(rusqlite::types::Value::Text(like.clone()));
            }
        }
        q += " WHERE 1=1";
        q += &where_clause;
        q += " ORDER BY p.composer, p.work, p.movement";
        let mut pieces = {
            let db = self.conn();
            let mut stmt = db.prepare(&q)?;

            stmt.query_map(rusqlite::params_from_iter(args.iter()), scan_piece)?
                .collect::<Result<Vec<_>>>()?
        };
        let today = today_str();
        for piece in &mut pieces {
            let cs = self.piece_auditions(piece.id)?;
            piece.has_active = cs.iter().any(|c| !c.archived && c.date >= today);
            piece.auditions = cs;
        }
        Ok(pieces)
    }

    pub fn get_piece(&self, id: i64) -> Result<Piece> {
        let mut piece = {
            let db = self.conn();
            db.query_row(
                "SELECT id, composer, work, movement, kind, difficulty, archived_at IS NOT NULL, target_bpm
				FROM pieces WHERE id=?",
                params![id],
                scan_piece,
            )?
        };
        piece.auditions = self.piece_auditions(id)?;
        Ok(piece)
    }

    /// The most recent declared confidence (0..5). A baseline
    /// declaration of 0 counts as rated; rated=false only when the
    /// piece has no valuation at all.
    pub fn latest_confidence(&self, piece_id: i64) -> Result<(f64, bool)> {
        let found: Option<Option<i64>> = self
            .conn()
            .query_row(
                "SELECT confidence FROM sessions
				WHERE piece_id=? AND (confidence>0 OR note='baseline') ORDER BY date DESC, id DESC LIMIT 1",
                params![piece_id],
                |row| row.get(0),
            )
            .optional()?;
        match found {
            None => Ok((0.0, false)),
            Some(value) => Ok((value.unwrap_or(0) as f64, true)),
        }
    }

    /// The most recent session date with real study time for the
    /// piece, or "" when it was never practiced.
    pub fn last_practice_date(&self, piece_id: i64) -> Result<String> {
        let found: Option<String> = self
            .conn()
            .query_row(
                "SELECT MAX(date) FROM sessions WHERE piece_id=? AND minutes>0",
                params![piece_id],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        Ok(found.unwrap_or_default())
    }

    /// Days since the last session with minutes>0; None when the
    /// piece was never practiced (the caller uses the default of 30).
    pub fn days_since_practice(&self, piece_id: i64, today: &str) -> Result<Option<i64>> {
        let last = self.last_practice_date(piece_id)?;
        if last.is_empty() {
            return Ok(None);
        }
        let n = days_between(&last, today)?;
        Ok(Some(n.max(0)))
    }

    /// Summarize today's sessions for a piece.
    pub fn today_mark(&self, piece_id: i64, today: &str) -> Result<TodayMark> {
        let db = self.conn();
        let mut stmt = db.prepare(
            "SELECT minutes, confidence, note FROM sessions WHERE piece_id=? AND date=?",
        )?;
        let rows = stmt
            .query_map(params![piece_id, today], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>>>()?;
        let mut mark = TodayMark::default();
        for (minutes, _confidence, note) in rows {
            mark.logged += minutes;
            if minutes > 0 {
                mark.practiced = true;
            }
            // 'postponed' is honored only for markers written before
            // the postpone feature was removed: they skip the piece
            // for today.
            if note == NOTE_SKIPPED || note == NOTE_POSTPONED {
                mark.skipped = true;
            }
        }
        Ok(mark)
    }

    pub fn availability_for(&self, date: &str) -> Result<Vec<Availability>> {
        let db = self.conn();
        let mut stmt = db.prepare(
            "SELECT id, date, start_min, end_min, label, kind FROM availability
			WHERE date=? ORDER BY start_min",
        )?;
        let rows = stmt
            .query_map(params![date], |row| {
                Ok(Availability {
                    id: row.get(0)?,
                    date: row.get(1)?,
                    start_min: row.get(2)?,
                    end_min: row.get(3)?,
                    label: row.get(4)?,
                    kind: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn get_coeffs(&self) -> Result<Coeffs> {
        let mut c = Coeffs::default();
        let db = self.conn();
        for (key, slot) in [
            ("urgency_k", &mut c.urgency_k),
            ("urgency_horizon", &mut c.urgency_horizon),
            ("recency_cap", &mut c.recency_cap),
            ("diff_horizon", &mut c.diff_horizon),
            ("diff_boost", &mut c.diff_boost),
        ] {
            let found: Option<String> = db
                .query_row(
                    "SELECT value FROM settings WHERE key=?",
                    params![key],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(text) = found
                && let Ok(f) = text.trim().parse::<f64>()
                && f > 0.0
            {
                *slot = f;
            }
        }
        Ok(c)
    }

    pub fn set_coeff(&self, key: &str, v: f64) -> Result<()> {
        self.conn().execute(
            "INSERT INTO settings(key,value) VALUES(?,?)
			ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, fmt_g(v)],
        )?;
        Ok(())
    }
}

fn scan_piece(row: &rusqlite::Row) -> Result<Piece> {
    Ok(Piece {
        id: row.get(0)?,
        composer: row.get(1)?,
        work: row.get(2)?,
        movement: row.get(3)?,
        kind: row.get(4)?,
        difficulty: row.get(5)?,
        archived: row.get::<_, i64>(6)? != 0,
        target_bpm: row.get(7)?,
        ..Default::default()
    })
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct TodayMark {
    pub logged: i64,
    /// A 'skipped' marker exists for today.
    pub skipped: bool,
    /// A session with real study time (minutes>0) exists for today.
    pub practiced: bool,
}

/// Format a float the way Go's %g verb does for the coefficient
/// values this application stores.
pub fn fmt_g(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app() -> App {
        App {
            db: Mutex::new(open_memory().unwrap()),
            data_dir: PathBuf::from("/tmp"),
        }
    }

    #[test]
    fn migrate_creates_the_schema_and_is_idempotent() {
        let app = test_app();
        let db = app.conn();
        for table in [
            "auditions",
            "pieces",
            "piece_audition",
            "sessions",
            "availability",
            "settings",
            "recordings",
            "mocks",
            "mock_items",
            "audition_rounds",
            "audition_links",
            "segments",
            "link_segments",
        ] {
            assert!(table_exists(&db, table).unwrap(), "missing {table}");
        }
        let cols = table_columns(&db, "pieces").unwrap();
        assert!(cols.contains("difficulty"));
        assert!(cols.contains("target_bpm"));
        let cols = table_columns(&db, "sessions").unwrap();
        assert!(cols.contains("segment_id"));
        drop(db);
        // A second migration pass changes nothing.
        let db = app.conn();
        migrate(&db).unwrap();
    }

    #[test]
    fn segments_round_trip_links_and_delete_guard() {
        let app = test_app();
        {
            let db = app.conn();
            db.execute(
                "INSERT INTO auditions(name, audition_date, weight) VALUES('Roma', '2026-12-06', 4)",
                [],
            )
            .unwrap();
            db.execute(
                "INSERT INTO pieces(composer, work, kind) VALUES('Haydn', 'Concerto in Re', 'solo')",
                [],
            )
            .unwrap();
            db.execute(
                "INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 1)",
                [],
            )
            .unwrap();
        }
        let cadenza = Segment {
            piece_id: 1,
            alias: "Cadenza".into(),
            start_kind: "bar".into(),
            start_value: "100".into(),
            end_kind: "bar".into(),
            end_value: "130".into(),
            ..Default::default()
        };
        let cadenza_id = app.add_segment(&cadenza).unwrap();
        let other_id = app
            .add_segment(&Segment {
                piece_id: 1,
                start_kind: "start".into(),
                end_kind: "bar".into(),
                end_value: "99".into(),
                ..Default::default()
            })
            .unwrap();
        // A duplicate alias on the same piece is refused.
        assert!(app.add_segment(&cadenza).is_err());
        // An unnamed segment may repeat.
        assert!(
            app.add_segment(&Segment {
                piece_id: 1,
                ..Default::default()
            })
            .is_ok()
        );
        let segs = app.segments_for_piece(1).unwrap();
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0].alias, "Cadenza");

        // Link set: a foreign segment id is ignored, members stick.
        app.set_link_segments(1, 1, &[cadenza_id, 999]).unwrap();
        assert_eq!(app.link_segment_ids(1, 1).unwrap(), vec![cadenza_id]);
        // Deletion is refused while a link references the segment.
        assert!(!app.delete_segment(cadenza_id).unwrap());
        app.set_link_segments(1, 1, &[]).unwrap();
        assert!(app.link_segment_ids(1, 1).unwrap().is_empty());
        assert!(app.delete_segment(cadenza_id).unwrap());
        assert!(app.delete_segment(other_id).unwrap());
        assert_eq!(app.segments_for_piece(1).unwrap().len(), 1);
    }

    #[test]
    fn pieces_round_trip_with_auditions() {
        let app = test_app();
        {
            let db = app.conn();
            db.execute(
                "INSERT INTO auditions(name, audition_date, weight) VALUES('Roma', '2026-12-06', 4)",
                [],
            )
            .unwrap();
            db.execute(
                "INSERT INTO pieces(composer, work, kind) VALUES('Haydn', 'Concerto in Do', 'solo')",
                [],
            )
            .unwrap();
            db.execute(
                "INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 1)",
                [],
            )
            .unwrap();
        }
        let pieces = app.list_pieces(0, "", false, "").unwrap();
        assert_eq!(pieces.len(), 1);
        assert_eq!(pieces[0].composer, "Haydn");
        assert_eq!(pieces[0].auditions.len(), 1);
        assert_eq!(app.list_pieces(0, "", false, "hayd").unwrap().len(), 1);
        assert_eq!(app.list_pieces(0, "", false, "zzz").unwrap().len(), 0);
        let piece = app.get_piece(1).unwrap();
        assert_eq!(piece.auditions[0].name, "Roma");
    }

    #[test]
    fn confidence_and_practice_dates() {
        let app = test_app();
        {
            let db = app.conn();
            db.execute("INSERT INTO pieces(composer, work) VALUES('A', 'B')", [])
                .unwrap();
            db.execute(
                "INSERT INTO sessions(date, piece_id, minutes, confidence) VALUES('2026-10-10', 1, 30, 4)",
                [],
            )
            .unwrap();
        }
        let (conf, rated) = app.latest_confidence(1).unwrap();
        assert!(rated && (conf - 4.0).abs() < f64::EPSILON);
        assert_eq!(app.last_practice_date(1).unwrap(), "2026-10-10");
        assert_eq!(app.days_since_practice(1, "2026-10-15").unwrap(), Some(5));
        let mark = app.today_mark(1, "2026-10-10").unwrap();
        assert_eq!(mark.logged, 30);
        assert!(mark.practiced);
    }

    #[test]
    fn coeffs_default_and_override() {
        let app = test_app();
        let c = app.get_coeffs().unwrap();
        assert_eq!(c, Coeffs::default());
        app.set_coeff("urgency_k", 3.5).unwrap();
        let c = app.get_coeffs().unwrap();
        assert!((c.urgency_k - 3.5).abs() < f64::EPSILON);
        assert_eq!(fmt_g(2.0), "2");
        assert_eq!(fmt_g(0.075), "0.075");
    }

    #[test]
    fn days_between_counts_whole_days() {
        assert_eq!(days_between("2026-01-01", "2026-01-02").unwrap(), 1);
        assert_eq!(days_between("2026-01-02", "2026-01-01").unwrap(), -1);
        assert!(days_between("nope", "2026-01-01").is_err());
    }
}
