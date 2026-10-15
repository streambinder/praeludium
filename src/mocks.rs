//! Mock auditions: a random draw of pieces from one audition,
//! played once each, in order, with a self rating per piece.

use crate::db::{App, Result};
use crate::i18n::decimal;
use rusqlite::params;
use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
pub struct Mock {
    #[serde(rename = "ID")]
    pub id: i64,
    #[serde(rename = "AuditionID")]
    pub audition_id: i64,
    #[serde(rename = "RoundID", serialize_with = "crate::db::ser_null_i64")]
    pub round_id: Option<i64>,
    #[serde(rename = "RoundName")]
    pub round_name: String,
    #[serde(rename = "Date")]
    pub date: String,
    #[serde(rename = "RecordTake")]
    pub record_take: bool,
    #[serde(rename = "Items")]
    pub items: Vec<MockItem>,
    #[serde(rename = "Rated")]
    pub rated: i64,
    #[serde(rename = "Mean")]
    pub mean: f64,
    /// Localized mean, stamped by handlers that render it.
    #[serde(rename = "MeanText")]
    pub mean_text: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct MockItem {
    #[serde(rename = "ID")]
    pub id: i64,
    #[serde(rename = "MockID")]
    pub mock_id: i64,
    #[serde(rename = "PieceID")]
    pub piece_id: i64,
    #[serde(rename = "Position")]
    pub position: i64,
    /// 0 until rated; rating is final (one take).
    #[serde(rename = "Confidence")]
    pub confidence: i64,
    #[serde(rename = "Note")]
    pub note: String,
    /// Composer — work, stamped from the piece.
    #[serde(rename = "Title")]
    pub title: String,
    #[serde(rename = "Movement")]
    pub movement: String,
    /// The excerpt this audition asks for.
    #[serde(rename = "Excerpt")]
    pub excerpt: String,
}

impl App {
    pub fn mock_by_id(&self, id: i64) -> Result<Mock> {
        let (mut mock, items) = {
            let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
            let mut m = db.query_row(
                "SELECT m.id, m.audition_id, m.round_id, COALESCE(rd.name, ''), m.date, m.record_take
				FROM mocks m LEFT JOIN audition_rounds rd ON rd.id = m.round_id WHERE m.id=?",
                params![id],
                |row| {
                    Ok(Mock {
                        id: row.get(0)?,
                        audition_id: row.get(1)?,
                        round_id: row.get(2)?,
                        round_name: row.get(3)?,
                        date: row.get(4)?,
                        record_take: row.get::<_, i64>(5)? != 0,
                        ..Default::default()
                    })
                },
            )?;
            let mut stmt = db.prepare(
                "SELECT mi.id, mi.mock_id, mi.piece_id, mi.position, mi.confidence, mi.note,
				p.composer, p.work, p.movement, COALESCE(pc.excerpt, '')
				FROM mock_items mi
				JOIN pieces p ON p.id = mi.piece_id
				LEFT JOIN piece_audition pc ON pc.piece_id = mi.piece_id AND pc.audition_id = ?
				WHERE mi.mock_id=? ORDER BY mi.position",
            )?;
            let items = stmt
                .query_map(params![m.audition_id, id], |row| {
                    let composer: String = row.get(6)?;
                    let work: String = row.get(7)?;
                    Ok(MockItem {
                        id: row.get(0)?,
                        mock_id: row.get(1)?,
                        piece_id: row.get(2)?,
                        position: row.get(3)?,
                        confidence: row.get(4)?,
                        note: row.get(5)?,
                        title: format!("{composer} — {work}"),
                        movement: row.get(8)?,
                        excerpt: row.get(9)?,
                    })
                })?
                .collect::<Result<Vec<_>>>()?;
            m.items = items.clone();
            (m, items)
        };
        let mut sum = 0i64;
        for it in &items {
            if it.confidence > 0 {
                mock.rated += 1;
                sum += it.confidence;
            }
        }
        if mock.rated > 0 {
            mock.mean = sum as f64 / mock.rated as f64;
        }
        Ok(mock)
    }

    /// List the mocks of one audition, newest first.
    pub fn audition_mocks(&self, audition_id: i64) -> Result<Vec<Mock>> {
        let ids = {
            let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
            let mut stmt =
                db.prepare("SELECT id FROM mocks WHERE audition_id=? ORDER BY date DESC, id DESC")?;

            stmt.query_map(params![audition_id], |row| row.get::<_, i64>(0))?
                .collect::<Result<Vec<_>>>()?
        };
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            out.push(self.mock_by_id(id)?);
        }
        Ok(out)
    }

    /// The pieces a mock can draw from: linked pieces of the
    /// audition, restricted to one round when a round is chosen. A
    /// piece with no round is asked in every round, so it always
    /// belongs.
    pub fn mock_pool(&self, audition_id: i64, round_id: Option<i64>) -> Result<Vec<i64>> {
        let db = self.db.lock().unwrap_or_else(|p| p.into_inner());
        let out = match round_id {
            Some(round) => {
                let mut stmt = db.prepare(
                    "SELECT pc.piece_id FROM piece_audition pc JOIN pieces p ON p.id = pc.piece_id
					WHERE pc.audition_id=? AND p.archived_at IS NULL AND (pc.round_id IS NULL OR pc.round_id=?)",
                )?;
                stmt.query_map(params![audition_id, round], |row| row.get::<_, i64>(0))?
                    .collect::<Result<Vec<_>>>()?
            }
            None => {
                let mut stmt = db.prepare(
                    "SELECT pc.piece_id FROM piece_audition pc JOIN pieces p ON p.id = pc.piece_id
					WHERE pc.audition_id=? AND p.archived_at IS NULL",
                )?;
                stmt.query_map(params![audition_id], |row| row.get::<_, i64>(0))?
                    .collect::<Result<Vec<_>>>()?
            }
        };
        Ok(out)
    }
}

/// Fill the localized mean label used by list views.
pub fn stamp_mock_means(mocks: &mut [Mock], lang: &str) {
    for m in mocks.iter_mut() {
        m.mean_text = decimal(lang, m.mean);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_memory;
    use std::sync::Mutex;

    fn test_app() -> App {
        let app = App {
            db: Mutex::new(open_memory().unwrap()),
            data_dir: std::path::PathBuf::from("/tmp"),
        };
        let db = app.db.lock().unwrap();
        db.execute(
            "INSERT INTO auditions(name, audition_date) VALUES('Roma', '2026-12-06')",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO pieces(composer, work) VALUES('Haydn', 'Concerto')",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO pieces(composer, work) VALUES('Dvorak', 'Concerto')",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO piece_audition(piece_id, audition_id) VALUES(1, 1)",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO piece_audition(piece_id, audition_id) VALUES(2, 1)",
            [],
        )
        .unwrap();
        drop(db);
        app
    }

    #[test]
    fn pool_and_mock_round_trip() {
        let app = test_app();
        let pool = app.mock_pool(1, None).unwrap();
        assert_eq!(pool.len(), 2);
        {
            let db = app.db.lock().unwrap();
            db.execute(
                "INSERT INTO mocks(audition_id, date, record_take) VALUES(1, '2026-10-14', 1)",
                [],
            )
            .unwrap();
            db.execute("INSERT INTO mock_items(mock_id, piece_id, position, confidence) VALUES(1, 1, 1, 4)", []).unwrap();
            db.execute(
                "INSERT INTO mock_items(mock_id, piece_id, position) VALUES(1, 2, 2)",
                [],
            )
            .unwrap();
        }
        let mock = app.mock_by_id(1).unwrap();
        assert!(mock.record_take);
        assert_eq!(mock.items.len(), 2);
        assert_eq!(mock.items[0].title, "Haydn — Concerto");
        assert_eq!(mock.rated, 1);
        assert!((mock.mean - 4.0).abs() < f64::EPSILON);
        let mocks = app.audition_mocks(1).unwrap();
        assert_eq!(mocks.len(), 1);
    }
}
