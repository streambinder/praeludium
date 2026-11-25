//! Repository seed: auditions, pieces, and segments as canonical
//! data, applied to the database at application start.
//!
//! The seed data lives in `crate::seed_data` as plain Rust data
//! with explicit IDs. The seed step upserts every seeded row
//! (the repository wins on conflict), never deletes a row, and
//! never touches `created_at`, users, enrollments, sessions, or
//! any other user data. The data types use `&'static str` so the
//! compiled `SEED` constant holds them directly; the database
//! structs carry owned `String`s and cannot sit in a constant.

use crate::db::Result;
use rusqlite::{Connection, OptionalExtension, params};

/// One seeded audition row: every column the seed writes.
/// `city` is seed data; the `Audition` struct does not expose it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeedAudition {
    pub id: i64,
    pub name: &'static str,
    pub city: &'static str,
    pub date: &'static str,
    pub weight: i64,
}

/// One seeded piece row: every column the seed writes. The
/// `Piece` struct does not expose `excerpt`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeedPiece {
    pub id: i64,
    pub composer: &'static str,
    pub work: &'static str,
    pub movement: &'static str,
    pub excerpt: &'static str,
    pub kind: &'static str,
    pub difficulty: i64,
    pub target_bpm: Option<i64>,
}

/// One seeded audition round row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeedRound {
    pub id: i64,
    pub audition_id: i64,
    pub position: i64,
    pub name: &'static str,
}

/// One seeded audition link row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeedLink {
    pub id: i64,
    pub audition_id: i64,
    pub label: &'static str,
    pub url: &'static str,
}

/// One seeded piece-audition link row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeedPieceAudition {
    pub piece_id: i64,
    pub audition_id: i64,
    pub excerpt: &'static str,
    pub note: &'static str,
    pub round_id: Option<i64>,
    pub optional: bool,
    pub cluster: Option<&'static str>,
}

/// One seeded segment row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeedSegment {
    pub id: i64,
    pub piece_id: i64,
    pub alias: &'static str,
    pub start_kind: &'static str,
    pub start_value: &'static str,
    pub start_offset: i64,
    pub end_kind: &'static str,
    pub end_value: &'static str,
    pub end_offset: i64,
}

/// One seeded link-segment row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeedLinkSegment {
    pub piece_id: i64,
    pub audition_id: i64,
    pub segment_id: i64,
}

/// Current seed columns of one piece row, for the overwrite check.
type PieceRow = (String, String, String, String, String, i64, Option<i64>);

/// Current seed columns of one segment row, for the overwrite check.
type SegmentRow = (i64, String, String, String, i64, String, String, i64);

/// Current seed columns of one link row, for the overwrite check.
type LinkRow = (String, String, Option<i64>, i64, Option<String>);

/// The seed data: one slice per canonical table.
#[derive(Debug, Default, PartialEq)]
pub struct SeedData {
    pub auditions: &'static [SeedAudition],
    pub pieces: &'static [SeedPiece],
    pub rounds: &'static [SeedRound],
    pub links: &'static [SeedLink],
    pub piece_auditions: &'static [SeedPieceAudition],
    pub segments: &'static [SeedSegment],
    pub link_segments: &'static [SeedLinkSegment],
}

/// What one seed application did to one table. Keys are row IDs
/// (`"7"`), or composite keys for the link tables (`"29:3"`,
/// `"25:1:22"`).
#[derive(Debug, Default, PartialEq)]
pub struct TableReport {
    pub inserted: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub overwritten: Vec<String>,
}

/// What one seed application did, per table.
#[derive(Debug, Default, PartialEq)]
pub struct SeedReport {
    pub auditions: TableReport,
    pub pieces: TableReport,
    pub rounds: TableReport,
    pub links: TableReport,
    pub piece_auditions: TableReport,
    pub segments: TableReport,
    pub link_segments: TableReport,
}

impl SeedReport {
    fn inserted(&self) -> usize {
        self.auditions.inserted
            + self.pieces.inserted
            + self.rounds.inserted
            + self.links.inserted
            + self.piece_auditions.inserted
            + self.segments.inserted
            + self.link_segments.inserted
    }

    fn updated(&self) -> usize {
        self.auditions.updated
            + self.pieces.updated
            + self.rounds.updated
            + self.links.updated
            + self.piece_auditions.updated
            + self.segments.updated
            + self.link_segments.updated
    }

    fn unchanged(&self) -> usize {
        self.auditions.unchanged
            + self.pieces.unchanged
            + self.rounds.unchanged
            + self.links.unchanged
            + self.piece_auditions.unchanged
            + self.segments.unchanged
            + self.link_segments.unchanged
    }

    fn overwritten(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = Vec::new();
        for table in [
            &self.auditions,
            &self.pieces,
            &self.rounds,
            &self.links,
            &self.piece_auditions,
            &self.segments,
            &self.link_segments,
        ] {
            keys.extend(table.overwritten.iter().map(String::as_str));
        }
        keys
    }
}

/// What the read-only comparison found in one table.
#[derive(Debug, Default, PartialEq)]
pub struct TableDiff {
    /// Seeded rows absent from the database.
    pub missing: Vec<String>,
    /// Database rows the seed does not hold.
    pub extra: Vec<String>,
    /// Rows present in both with different seed columns.
    pub different: Vec<String>,
}

/// What the read-only comparison found, per table.
#[derive(Debug, Default, PartialEq)]
pub struct SeedDiff {
    pub auditions: TableDiff,
    pub pieces: TableDiff,
    pub rounds: TableDiff,
    pub links: TableDiff,
    pub piece_auditions: TableDiff,
    pub segments: TableDiff,
    pub link_segments: TableDiff,
}

impl SeedDiff {
    fn missing(&self) -> usize {
        self.auditions.missing.len()
            + self.pieces.missing.len()
            + self.rounds.missing.len()
            + self.links.missing.len()
            + self.piece_auditions.missing.len()
            + self.segments.missing.len()
            + self.link_segments.missing.len()
    }

    fn extra(&self) -> usize {
        self.auditions.extra.len()
            + self.pieces.extra.len()
            + self.rounds.extra.len()
            + self.links.extra.len()
            + self.piece_auditions.extra.len()
            + self.segments.extra.len()
            + self.link_segments.extra.len()
    }

    fn different(&self) -> usize {
        self.auditions.different.len()
            + self.pieces.different.len()
            + self.rounds.different.len()
            + self.links.different.len()
            + self.piece_auditions.different.len()
            + self.segments.different.len()
            + self.link_segments.different.len()
    }
}

/// Apply the seed data to the database: upsert every seeded row
/// in one transaction, in foreign-key order. The repository wins
/// on conflict; a row that already matches the seed is left
/// untouched, so a second application writes nothing. Never
/// deletes a row, never touches `created_at`, never writes users,
/// enrollments, sessions, or any other user data. Logs the report
/// and the comparison counts at startup.
pub fn apply_seed(db: &mut Connection, data: &SeedData) -> Result<SeedReport> {
    let tx = db.transaction()?;
    let mut report = SeedReport::default();
    seed_auditions(&tx, data.auditions, &mut report.auditions)?;
    seed_pieces(&tx, data.pieces, &mut report.pieces)?;
    seed_rounds(&tx, data.rounds, &mut report.rounds)?;
    seed_links(&tx, data.links, &mut report.links)?;
    seed_piece_auditions(&tx, data.piece_auditions, &mut report.piece_auditions)?;
    seed_segments(&tx, data.segments, &mut report.segments)?;
    seed_link_segments(&tx, data.link_segments, &mut report.link_segments)?;
    tx.commit()?;
    let diff = compare_seed(db, data)?;
    eprintln!(
        "event seed applied inserted={} updated={} unchanged={} overwritten={:?}",
        report.inserted(),
        report.updated(),
        report.unchanged(),
        report.overwritten()
    );
    eprintln!(
        "event seed comparison missing={} extra={} different={}",
        diff.missing(),
        diff.extra(),
        diff.different()
    );
    Ok(report)
}

fn seed_auditions(
    tx: &rusqlite::Transaction<'_>,
    rows: &[SeedAudition],
    report: &mut TableReport,
) -> Result<()> {
    let mut existing =
        tx.prepare("SELECT name, city, audition_date, weight FROM auditions WHERE id=?")?;
    let mut upsert = tx.prepare(
        "INSERT INTO auditions(id, name, city, audition_date, weight)
        VALUES(?,?,?,?,?)
        ON CONFLICT(id) DO UPDATE SET name=excluded.name, city=excluded.city,
        audition_date=excluded.audition_date, weight=excluded.weight",
    )?;
    for row in rows {
        let found: Option<(String, String, String, i64)> = existing
            .query_row(params![row.id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })
            .optional()?;
        match found {
            None => {
                upsert.execute(params![row.id, row.name, row.city, row.date, row.weight])?;
                report.inserted += 1;
            }
            Some(current)
                if current
                    != (
                        row.name.to_string(),
                        row.city.to_string(),
                        row.date.to_string(),
                        row.weight,
                    ) =>
            {
                upsert.execute(params![row.id, row.name, row.city, row.date, row.weight])?;
                report.updated += 1;
                report.overwritten.push(row.id.to_string());
            }
            Some(_) => report.unchanged += 1,
        }
    }
    Ok(())
}

fn seed_pieces(
    tx: &rusqlite::Transaction<'_>,
    rows: &[SeedPiece],
    report: &mut TableReport,
) -> Result<()> {
    let mut existing = tx.prepare(
        "SELECT composer, work, movement, excerpt, kind, difficulty, target_bpm
        FROM pieces WHERE id=?",
    )?;
    let mut upsert = tx.prepare(
        "INSERT INTO pieces(id, composer, work, movement, excerpt, kind, difficulty, target_bpm)
        VALUES(?,?,?,?,?,?,?,?)
        ON CONFLICT(id) DO UPDATE SET composer=excluded.composer, work=excluded.work,
        movement=excluded.movement, excerpt=excluded.excerpt, kind=excluded.kind,
        difficulty=excluded.difficulty, target_bpm=excluded.target_bpm",
    )?;
    for row in rows {
        let found: Option<PieceRow> = existing
            .query_row(params![row.id], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                ))
            })
            .optional()?;
        let wanted = (
            row.composer.to_string(),
            row.work.to_string(),
            row.movement.to_string(),
            row.excerpt.to_string(),
            row.kind.to_string(),
            row.difficulty,
            row.target_bpm,
        );
        match found {
            None => {
                upsert.execute(params![
                    row.id,
                    row.composer,
                    row.work,
                    row.movement,
                    row.excerpt,
                    row.kind,
                    row.difficulty,
                    row.target_bpm
                ])?;
                report.inserted += 1;
            }
            Some(current) if current != wanted => {
                upsert.execute(params![
                    row.id,
                    row.composer,
                    row.work,
                    row.movement,
                    row.excerpt,
                    row.kind,
                    row.difficulty,
                    row.target_bpm
                ])?;
                report.updated += 1;
                report.overwritten.push(row.id.to_string());
            }
            Some(_) => report.unchanged += 1,
        }
    }
    Ok(())
}

fn seed_rounds(
    tx: &rusqlite::Transaction<'_>,
    rows: &[SeedRound],
    report: &mut TableReport,
) -> Result<()> {
    let mut existing =
        tx.prepare("SELECT audition_id, position, name FROM audition_rounds WHERE id=?")?;
    let mut upsert = tx.prepare(
        "INSERT INTO audition_rounds(id, audition_id, position, name) VALUES(?,?,?,?)
        ON CONFLICT(id) DO UPDATE SET audition_id=excluded.audition_id,
        position=excluded.position, name=excluded.name",
    )?;
    for row in rows {
        let found: Option<(i64, i64, String)> = existing
            .query_row(params![row.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .optional()?;
        let wanted = (row.audition_id, row.position, row.name.to_string());
        match found {
            None => {
                upsert.execute(params![row.id, row.audition_id, row.position, row.name])?;
                report.inserted += 1;
            }
            Some(current) if current != wanted => {
                upsert.execute(params![row.id, row.audition_id, row.position, row.name])?;
                report.updated += 1;
                report.overwritten.push(row.id.to_string());
            }
            Some(_) => report.unchanged += 1,
        }
    }
    Ok(())
}

fn seed_links(
    tx: &rusqlite::Transaction<'_>,
    rows: &[SeedLink],
    report: &mut TableReport,
) -> Result<()> {
    let mut existing =
        tx.prepare("SELECT audition_id, label, url FROM audition_links WHERE id=?")?;
    let mut upsert = tx.prepare(
        "INSERT INTO audition_links(id, audition_id, label, url) VALUES(?,?,?,?)
        ON CONFLICT(id) DO UPDATE SET audition_id=excluded.audition_id,
        label=excluded.label, url=excluded.url",
    )?;
    for row in rows {
        let found: Option<(i64, String, String)> = existing
            .query_row(params![row.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .optional()?;
        let wanted = (row.audition_id, row.label.to_string(), row.url.to_string());
        match found {
            None => {
                upsert.execute(params![row.id, row.audition_id, row.label, row.url])?;
                report.inserted += 1;
            }
            Some(current) if current != wanted => {
                upsert.execute(params![row.id, row.audition_id, row.label, row.url])?;
                report.updated += 1;
                report.overwritten.push(row.id.to_string());
            }
            Some(_) => report.unchanged += 1,
        }
    }
    Ok(())
}

fn seed_piece_auditions(
    tx: &rusqlite::Transaction<'_>,
    rows: &[SeedPieceAudition],
    report: &mut TableReport,
) -> Result<()> {
    let mut existing = tx.prepare(
        "SELECT excerpt, note, round_id, optional, cluster FROM piece_audition
        WHERE piece_id=? AND audition_id=?",
    )?;
    let mut upsert = tx.prepare(
        "INSERT INTO piece_audition(piece_id, audition_id, excerpt, note, round_id, optional, cluster)
        VALUES(?,?,?,?,?,?,?)
        ON CONFLICT(piece_id, audition_id) DO UPDATE SET excerpt=excluded.excerpt,
        note=excluded.note, round_id=excluded.round_id, optional=excluded.optional,
        cluster=excluded.cluster",
    )?;
    for row in rows {
        let found: Option<LinkRow> = existing
            .query_row(params![row.piece_id, row.audition_id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })
            .optional()?;
        let wanted = (
            row.excerpt.to_string(),
            row.note.to_string(),
            row.round_id,
            i64::from(row.optional),
            row.cluster.map(str::to_string),
        );
        let key = format!("{}:{}", row.piece_id, row.audition_id);
        match found {
            None => {
                upsert.execute(params![
                    row.piece_id,
                    row.audition_id,
                    row.excerpt,
                    row.note,
                    row.round_id,
                    i64::from(row.optional),
                    row.cluster
                ])?;
                report.inserted += 1;
            }
            Some(current) if current != wanted => {
                upsert.execute(params![
                    row.piece_id,
                    row.audition_id,
                    row.excerpt,
                    row.note,
                    row.round_id,
                    i64::from(row.optional),
                    row.cluster
                ])?;
                report.updated += 1;
                report.overwritten.push(key);
            }
            Some(_) => report.unchanged += 1,
        }
    }
    Ok(())
}

fn seed_segments(
    tx: &rusqlite::Transaction<'_>,
    rows: &[SeedSegment],
    report: &mut TableReport,
) -> Result<()> {
    let mut existing = tx.prepare(
        "SELECT piece_id, alias, start_kind, start_value, start_offset,
        end_kind, end_value, end_offset FROM segments WHERE id=?",
    )?;
    let mut upsert = tx.prepare(
        "INSERT INTO segments(id, piece_id, alias, start_kind, start_value, start_offset,
        end_kind, end_value, end_offset) VALUES(?,?,?,?,?,?,?,?,?)
        ON CONFLICT(id) DO UPDATE SET piece_id=excluded.piece_id, alias=excluded.alias,
        start_kind=excluded.start_kind, start_value=excluded.start_value,
        start_offset=excluded.start_offset, end_kind=excluded.end_kind,
        end_value=excluded.end_value, end_offset=excluded.end_offset",
    )?;
    for row in rows {
        let found: Option<SegmentRow> = existing
            .query_row(params![row.id], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                ))
            })
            .optional()?;
        let wanted = (
            row.piece_id,
            row.alias.to_string(),
            row.start_kind.to_string(),
            row.start_value.to_string(),
            row.start_offset,
            row.end_kind.to_string(),
            row.end_value.to_string(),
            row.end_offset,
        );
        match found {
            None => {
                upsert.execute(params![
                    row.id,
                    row.piece_id,
                    row.alias,
                    row.start_kind,
                    row.start_value,
                    row.start_offset,
                    row.end_kind,
                    row.end_value,
                    row.end_offset
                ])?;
                report.inserted += 1;
            }
            Some(current) if current != wanted => {
                upsert.execute(params![
                    row.id,
                    row.piece_id,
                    row.alias,
                    row.start_kind,
                    row.start_value,
                    row.start_offset,
                    row.end_kind,
                    row.end_value,
                    row.end_offset
                ])?;
                report.updated += 1;
                report.overwritten.push(row.id.to_string());
            }
            Some(_) => report.unchanged += 1,
        }
    }
    Ok(())
}

fn seed_link_segments(
    tx: &rusqlite::Transaction<'_>,
    rows: &[SeedLinkSegment],
    report: &mut TableReport,
) -> Result<()> {
    let mut existing = tx.prepare(
        "SELECT 1 FROM link_segments
        WHERE piece_id=? AND audition_id=? AND segment_id=?",
    )?;
    let mut insert = tx.prepare(
        "INSERT OR IGNORE INTO link_segments(piece_id, audition_id, segment_id)
        VALUES(?,?,?)",
    )?;
    for row in rows {
        let found: Option<i64> = existing
            .query_row(
                params![row.piece_id, row.audition_id, row.segment_id],
                |r| r.get(0),
            )
            .optional()?;
        match found {
            None => {
                insert.execute(params![row.piece_id, row.audition_id, row.segment_id])?;
                report.inserted += 1;
            }
            Some(_) => report.unchanged += 1,
        }
    }
    Ok(())
}

/// Compare the database against the seed data, with pure reads.
/// Reports the seeded rows missing from the database, the
/// database rows the seed does not hold, and the rows present in
/// both with different seed columns. `created_at` is never a seed
/// column, so it never counts as different.
pub fn compare_seed(db: &Connection, data: &SeedData) -> Result<SeedDiff> {
    let mut diff = SeedDiff::default();
    compare_auditions(db, data.auditions, &mut diff.auditions)?;
    compare_pieces(db, data.pieces, &mut diff.pieces)?;
    compare_rounds(db, data.rounds, &mut diff.rounds)?;
    compare_links(db, data.links, &mut diff.links)?;
    compare_piece_auditions(db, data.piece_auditions, &mut diff.piece_auditions)?;
    compare_segments(db, data.segments, &mut diff.segments)?;
    compare_link_segments(db, data.link_segments, &mut diff.link_segments)?;
    Ok(diff)
}

fn compare_auditions(db: &Connection, rows: &[SeedAudition], diff: &mut TableDiff) -> Result<()> {
    let mut stmt = db.prepare("SELECT id, name, city, audition_date, weight FROM auditions")?;
    let stored = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                (
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i64>(4)?,
                ),
            ))
        })?
        .collect::<Result<std::collections::HashMap<i64, (String, String, String, i64)>>>()?;
    for row in rows {
        let wanted = (
            row.name.to_string(),
            row.city.to_string(),
            row.date.to_string(),
            row.weight,
        );
        match stored.get(&row.id) {
            None => diff.missing.push(row.id.to_string()),
            Some(current) if *current != wanted => diff.different.push(row.id.to_string()),
            Some(_) => {}
        }
    }
    for id in stored.keys() {
        if !rows.iter().any(|row| row.id == *id) {
            diff.extra.push(id.to_string());
        }
    }
    Ok(())
}

fn compare_pieces(db: &Connection, rows: &[SeedPiece], diff: &mut TableDiff) -> Result<()> {
    let mut stmt = db.prepare(
        "SELECT id, composer, work, movement, excerpt, kind, difficulty, target_bpm
        FROM pieces",
    )?;
    let stored = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                (
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, i64>(6)?,
                    r.get::<_, Option<i64>>(7)?,
                ),
            ))
        })?
        .collect::<Result<
            std::collections::HashMap<
                i64,
                (String, String, String, String, String, i64, Option<i64>),
            >,
        >>()?;
    for row in rows {
        let wanted = (
            row.composer.to_string(),
            row.work.to_string(),
            row.movement.to_string(),
            row.excerpt.to_string(),
            row.kind.to_string(),
            row.difficulty,
            row.target_bpm,
        );
        match stored.get(&row.id) {
            None => diff.missing.push(row.id.to_string()),
            Some(current) if *current != wanted => diff.different.push(row.id.to_string()),
            Some(_) => {}
        }
    }
    for id in stored.keys() {
        if !rows.iter().any(|row| row.id == *id) {
            diff.extra.push(id.to_string());
        }
    }
    Ok(())
}

fn compare_rounds(db: &Connection, rows: &[SeedRound], diff: &mut TableDiff) -> Result<()> {
    let mut stmt = db.prepare("SELECT id, audition_id, position, name FROM audition_rounds")?;
    let stored = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                (
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, String>(3)?,
                ),
            ))
        })?
        .collect::<Result<std::collections::HashMap<i64, (i64, i64, String)>>>()?;
    for row in rows {
        let wanted = (row.audition_id, row.position, row.name.to_string());
        match stored.get(&row.id) {
            None => diff.missing.push(row.id.to_string()),
            Some(current) if *current != wanted => diff.different.push(row.id.to_string()),
            Some(_) => {}
        }
    }
    for id in stored.keys() {
        if !rows.iter().any(|row| row.id == *id) {
            diff.extra.push(id.to_string());
        }
    }
    Ok(())
}

fn compare_links(db: &Connection, rows: &[SeedLink], diff: &mut TableDiff) -> Result<()> {
    let mut stmt = db.prepare("SELECT id, audition_id, label, url FROM audition_links")?;
    let stored = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                (
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ),
            ))
        })?
        .collect::<Result<std::collections::HashMap<i64, (i64, String, String)>>>()?;
    for row in rows {
        let wanted = (row.audition_id, row.label.to_string(), row.url.to_string());
        match stored.get(&row.id) {
            None => diff.missing.push(row.id.to_string()),
            Some(current) if *current != wanted => diff.different.push(row.id.to_string()),
            Some(_) => {}
        }
    }
    for id in stored.keys() {
        if !rows.iter().any(|row| row.id == *id) {
            diff.extra.push(id.to_string());
        }
    }
    Ok(())
}

fn compare_piece_auditions(
    db: &Connection,
    rows: &[SeedPieceAudition],
    diff: &mut TableDiff,
) -> Result<()> {
    let mut stmt = db.prepare(
        "SELECT piece_id, audition_id, excerpt, note, round_id, optional, cluster FROM piece_audition",
    )?;
    let stored = stmt
        .query_map([], |r| {
            Ok((
                format!("{}:{}", r.get::<_, i64>(0)?, r.get::<_, i64>(1)?),
                (
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<i64>>(4)?,
                    r.get::<_, i64>(5)?,
                    r.get::<_, Option<String>>(6)?,
                ),
            ))
        })?
        .collect::<Result<std::collections::HashMap<String, LinkRow>>>()?;
    for row in rows {
        let key = format!("{}:{}", row.piece_id, row.audition_id);
        let wanted = (
            row.excerpt.to_string(),
            row.note.to_string(),
            row.round_id,
            i64::from(row.optional),
            row.cluster.map(str::to_string),
        );
        match stored.get(&key) {
            None => diff.missing.push(key),
            Some(current) if *current != wanted => diff.different.push(key),
            Some(_) => {}
        }
    }
    for key in stored.keys() {
        if !rows
            .iter()
            .any(|row| format!("{}:{}", row.piece_id, row.audition_id) == *key)
        {
            diff.extra.push(key.clone());
        }
    }
    Ok(())
}

fn compare_segments(db: &Connection, rows: &[SeedSegment], diff: &mut TableDiff) -> Result<()> {
    let mut stmt = db.prepare(
        "SELECT id, piece_id, alias, start_kind, start_value, start_offset,
        end_kind, end_value, end_offset FROM segments",
    )?;
    let stored = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                (
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, i64>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, String>(7)?,
                    r.get::<_, i64>(8)?,
                ),
            ))
        })?
        .collect::<Result<
            std::collections::HashMap<i64, (i64, String, String, String, i64, String, String, i64)>,
        >>()?;
    for row in rows {
        let wanted = (
            row.piece_id,
            row.alias.to_string(),
            row.start_kind.to_string(),
            row.start_value.to_string(),
            row.start_offset,
            row.end_kind.to_string(),
            row.end_value.to_string(),
            row.end_offset,
        );
        match stored.get(&row.id) {
            None => diff.missing.push(row.id.to_string()),
            Some(current) if *current != wanted => diff.different.push(row.id.to_string()),
            Some(_) => {}
        }
    }
    for id in stored.keys() {
        if !rows.iter().any(|row| row.id == *id) {
            diff.extra.push(id.to_string());
        }
    }
    Ok(())
}

fn compare_link_segments(
    db: &Connection,
    rows: &[SeedLinkSegment],
    diff: &mut TableDiff,
) -> Result<()> {
    let mut stmt = db.prepare("SELECT piece_id, audition_id, segment_id FROM link_segments")?;
    let stored = stmt
        .query_map([], |r| {
            Ok(format!(
                "{}:{}:{}",
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?
            ))
        })?
        .collect::<Result<std::collections::HashSet<String>>>()?;
    for row in rows {
        let key = format!("{}:{}:{}", row.piece_id, row.audition_id, row.segment_id);
        if !stored.contains(&key) {
            diff.missing.push(key);
        }
    }
    for key in &stored {
        if !rows
            .iter()
            .any(|row| format!("{}:{}:{}", row.piece_id, row.audition_id, row.segment_id) == *key)
        {
            diff.extra.push(key.clone());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{SEEDED_USER_ID, ensure_test_user, open_db, open_memory};
    use crate::seed_data::SEED;

    fn count(db: &Connection, table: &str) -> i64 {
        db.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }

    /// Canonical text dump of every table the seed and the user
    /// data touch, one row per line, in primary-key order.
    fn dump(db: &Connection) -> String {
        let tables: &[(&str, &str)] = &[
            ("auditions", "id"),
            ("pieces", "id"),
            ("audition_rounds", "id"),
            ("audition_links", "id"),
            ("piece_audition", "piece_id, audition_id"),
            ("segments", "id"),
            ("link_segments", "piece_id, audition_id, segment_id"),
            ("users", "id"),
            ("enrollments", "user_id, audition_id"),
            ("sessions", "id"),
        ];
        let mut out = String::new();
        for (table, order) in tables {
            out.push_str(table);
            out.push('\n');
            let mut stmt = db
                .prepare(&format!("SELECT * FROM {table} ORDER BY {order}"))
                .unwrap();
            let cols = stmt.column_count();
            let rows = stmt
                .query_map([], |row| {
                    let mut vals = Vec::new();
                    for i in 0..cols {
                        vals.push(format!(
                            "{:?}",
                            row.get::<_, rusqlite::types::Value>(i).unwrap()
                        ));
                    }
                    Ok(vals.join("|"))
                })
                .unwrap();
            for row in rows {
                out.push_str(&row.unwrap());
                out.push('\n');
            }
        }
        out
    }

    fn temp_path(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("praeludium-seed-{tag}-{}.db", std::process::id()))
    }

    fn cleanup(path: &std::path::Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(format!("{}-wal", path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", path.display()));
    }

    /// 5.1: a fresh database holds the seed, with the IDs of the dump.
    #[test]
    fn fresh_database_holds_the_seed() {
        let db = open_memory().unwrap();
        assert_eq!(count(&db, "auditions"), 6);
        assert_eq!(count(&db, "audition_links"), 18);
        assert_eq!(count(&db, "audition_rounds"), 13);
        assert_eq!(count(&db, "pieces"), 46);
        assert_eq!(count(&db, "piece_audition"), 97);
        assert_eq!(count(&db, "segments"), 25);
        assert_eq!(count(&db, "link_segments"), 35);
        assert_eq!(count(&db, "users"), 0);
        assert_eq!(count(&db, "enrollments"), 0);
        // Three choice clusters hold seven links: Haydn and
        // concerto for San Carlo, concerto for La Fenice. The
        // eighth optional link, Piatti for Santa Cecilia, has
        // no cluster.
        let clustered: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM piece_audition WHERE cluster IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(clustered, 7);
        let groups: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM (SELECT DISTINCT audition_id, cluster FROM piece_audition WHERE cluster IS NOT NULL)",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(groups, 3);
        let composer: String = db
            .query_row("SELECT composer FROM pieces WHERE id=1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(composer, "Beethoven");
        let name: String = db
            .query_row("SELECT name FROM auditions WHERE id=6", [], |r| r.get(0))
            .unwrap();
        assert_eq!(name, "La Fenice — Violoncello di fila");
        let diff = compare_seed(&db, &SEED).unwrap();
        assert_eq!(diff.missing(), 0);
        assert_eq!(diff.extra(), 0);
        assert_eq!(diff.different(), 0);
    }

    /// 5.2: a file database opened twice yields an identical dump.
    #[test]
    fn second_open_changes_nothing() {
        let path = temp_path("second-open");
        cleanup(&path);
        let first = {
            let db = open_db(&path).unwrap();
            let dump = dump(&db);
            drop(db);
            dump
        };
        let db = open_db(&path).unwrap();
        let second = dump(&db);
        drop(db);
        cleanup(&path);
        assert_eq!(first, second);
    }

    /// 5.3: an overwrite is reported and applied on reopen.
    #[test]
    fn overwrite_is_reported_and_applied() {
        let path = temp_path("overwrite");
        cleanup(&path);
        let mut db = open_db(&path).unwrap();
        db.execute("UPDATE pieces SET difficulty=1 WHERE id=1", [])
            .unwrap();
        let report = apply_seed(&mut db, &SEED).unwrap();
        assert_eq!(report.pieces.updated, 1);
        assert_eq!(report.pieces.overwritten, vec!["1".to_string()]);
        let difficulty: i64 = db
            .query_row("SELECT difficulty FROM pieces WHERE id=1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(difficulty, 4);
        drop(db);
        let db = open_db(&path).unwrap();
        let difficulty: i64 = db
            .query_row("SELECT difficulty FROM pieces WHERE id=1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(difficulty, 4);
        let diff = compare_seed(&db, &SEED).unwrap();
        assert_eq!(diff.different(), 0);
        drop(db);
        cleanup(&path);
    }

    /// 5.4: a row the seed does not hold stays and is reported.
    #[test]
    fn extra_row_stays_and_is_reported() {
        let path = temp_path("extra-row");
        cleanup(&path);
        {
            let db = open_db(&path).unwrap();
            db.execute(
                "INSERT INTO auditions(id, name, city, audition_date, weight)
                VALUES(7, 'Extra', '', '2027-03-01', 2)",
                [],
            )
            .unwrap();
        }
        let db = open_db(&path).unwrap();
        let name: String = db
            .query_row("SELECT name FROM auditions WHERE id=7", [], |r| r.get(0))
            .unwrap();
        assert_eq!(name, "Extra");
        assert_eq!(count(&db, "auditions"), 7);
        let diff = compare_seed(&db, &SEED).unwrap();
        assert_eq!(diff.auditions.extra, vec!["7".to_string()]);
        drop(db);
        cleanup(&path);
    }

    /// 5.5: sessions and enrollments survive a reopen.
    #[test]
    fn sessions_and_enrollments_survive() {
        let path = temp_path("user-data");
        cleanup(&path);
        let session_id = {
            let db = open_db(&path).unwrap();
            ensure_test_user(&db).unwrap();
            db.execute(
                "INSERT INTO sessions(date, piece_id, minutes, confidence, note)
                VALUES('2026-11-20', 1, 30, 4, 'kept')",
                [],
            )
            .unwrap();
            let id = db.last_insert_rowid();
            db.execute(
                "INSERT INTO enrollments(user_id, audition_id) VALUES(?, 1)",
                params![SEEDED_USER_ID],
            )
            .unwrap();
            drop(db);
            id
        };
        let db = open_db(&path).unwrap();
        let (piece, minutes, confidence, note): (i64, i64, i64, String) = db
            .query_row(
                "SELECT piece_id, minutes, confidence, note FROM sessions WHERE id=?",
                params![session_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!((piece, minutes, confidence), (1, 30, 4));
        assert_eq!(note, "kept");
        assert_eq!(count(&db, "sessions"), 1);
        let enrolled: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM enrollments WHERE user_id=? AND audition_id=1",
                params![SEEDED_USER_ID],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(enrolled, 1);
        drop(db);
        cleanup(&path);
    }

    /// 5.7: a seed addition through a custom instance applies
    /// once, and a second apply changes nothing.
    #[test]
    fn seed_addition_through_a_custom_instance() {
        static EXTRA_AUDITIONS: &[SeedAudition] = &[SeedAudition {
            id: 7,
            name: "Extra",
            city: "",
            date: "2027-03-01",
            weight: 2,
        }];
        let extra = SeedData {
            auditions: EXTRA_AUDITIONS,
            ..Default::default()
        };
        let mut db = open_memory().unwrap();
        let first = apply_seed(&mut db, &extra).unwrap();
        assert_eq!(first.auditions.inserted, 1);
        assert_eq!(count(&db, "auditions"), 7);
        let second = apply_seed(&mut db, &extra).unwrap();
        assert_eq!(second.auditions.inserted, 0);
        assert_eq!(second.auditions.updated, 0);
        assert_eq!(second.auditions.unchanged, 1);
        let name: String = db
            .query_row("SELECT name FROM auditions WHERE id=7", [], |r| r.get(0))
            .unwrap();
        assert_eq!(name, "Extra");
        assert_eq!(count(&db, "pieces"), 46);
    }

    /// Every table, one row deleted and one row changed: the
    /// comparison reports the missing and the different rows,
    /// and a seed application restores them and reports the
    /// overwrites.
    #[test]
    fn comparison_and_overwrite_cover_every_table() {
        let mut db = open_memory().unwrap();
        db.execute("DELETE FROM auditions WHERE id=2", []).unwrap();
        db.execute("DELETE FROM pieces WHERE id=45", []).unwrap();
        db.execute("DELETE FROM audition_rounds WHERE id=1", [])
            .unwrap();
        db.execute("DELETE FROM audition_links WHERE id=1", [])
            .unwrap();
        db.execute(
            "DELETE FROM piece_audition WHERE piece_id=29 AND audition_id=3",
            [],
        )
        .unwrap();
        db.execute("DELETE FROM segments WHERE id=1", []).unwrap();
        db.execute(
            "DELETE FROM link_segments WHERE piece_id=25 AND audition_id=1 AND segment_id=22",
            [],
        )
        .unwrap();
        db.execute("UPDATE auditions SET weight=weight+100 WHERE id=1", [])
            .unwrap();
        db.execute("UPDATE pieces SET difficulty=difficulty+100 WHERE id=2", [])
            .unwrap();
        db.execute("UPDATE audition_rounds SET name=name || 'x' WHERE id=2", [])
            .unwrap();
        db.execute(
            "UPDATE audition_links SET label=label || 'x' WHERE id=2",
            [],
        )
        .unwrap();
        db.execute(
            "UPDATE piece_audition SET note=note || 'x' WHERE piece_id=30 AND audition_id=3",
            [],
        )
        .unwrap();
        db.execute("UPDATE segments SET alias=alias || 'x' WHERE id=2", [])
            .unwrap();
        let diff = compare_seed(&db, &SEED).unwrap();
        assert!(diff.auditions.missing.contains(&"2".to_string()));
        assert!(diff.pieces.missing.contains(&"45".to_string()));
        assert!(diff.rounds.missing.contains(&"1".to_string()));
        assert!(diff.links.missing.contains(&"1".to_string()));
        assert!(diff.piece_auditions.missing.contains(&"29:3".to_string()));
        assert!(diff.segments.missing.contains(&"1".to_string()));
        assert!(diff.link_segments.missing.contains(&"25:1:22".to_string()));
        assert!(diff.auditions.different.contains(&"1".to_string()));
        assert!(diff.pieces.different.contains(&"2".to_string()));
        assert!(diff.rounds.different.contains(&"2".to_string()));
        assert!(diff.links.different.contains(&"2".to_string()));
        assert!(diff.piece_auditions.different.contains(&"30:3".to_string()));
        assert!(diff.segments.different.contains(&"2".to_string()));
        let report = apply_seed(&mut db, &SEED).unwrap();
        assert_eq!(report.auditions.updated, 1);
        assert_eq!(report.pieces.updated, 1);
        assert_eq!(report.rounds.updated, 1);
        assert_eq!(report.links.updated, 1);
        assert_eq!(report.piece_auditions.updated, 1);
        assert_eq!(report.segments.updated, 1);
        assert_eq!(report.auditions.overwritten, vec!["1".to_string()]);
        assert_eq!(report.pieces.overwritten, vec!["2".to_string()]);
        assert_eq!(report.rounds.overwritten, vec!["2".to_string()]);
        assert_eq!(report.links.overwritten, vec!["2".to_string()]);
        assert_eq!(report.piece_auditions.overwritten, vec!["30:3".to_string()]);
        assert_eq!(report.segments.overwritten, vec!["2".to_string()]);
        assert!(report.auditions.inserted >= 1);
        assert!(report.pieces.inserted >= 1);
        assert!(report.rounds.inserted >= 1);
        assert!(report.links.inserted >= 1);
        assert!(report.piece_auditions.inserted >= 1);
        assert!(report.segments.inserted >= 1);
        assert!(report.link_segments.inserted >= 1);
        let clean = compare_seed(&db, &SEED).unwrap();
        assert_eq!(clean.missing(), 0);
        assert_eq!(clean.extra(), 0);
        assert_eq!(clean.different(), 0);
    }

    /// 5.9: `created_at` survives an overwrite of a seeded row.
    #[test]
    fn created_at_survives_an_overwrite() {
        let path = temp_path("created-at");
        cleanup(&path);
        let created = {
            let db = open_db(&path).unwrap();
            let created: String = db
                .query_row("SELECT created_at FROM pieces WHERE id=1", [], |r| r.get(0))
                .unwrap();
            db.execute("UPDATE pieces SET difficulty=1 WHERE id=1", [])
                .unwrap();
            drop(db);
            created
        };
        let db = open_db(&path).unwrap();
        let (difficulty, kept): (i64, String) = db
            .query_row(
                "SELECT difficulty, created_at FROM pieces WHERE id=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(difficulty, 4);
        assert_eq!(kept, created);
        drop(db);
        cleanup(&path);
    }
}
