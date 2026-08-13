use crate::models::*;
use anyhow::{bail, Context, Result};
use base64::Engine;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub struct Database {
    conn: Connection,
    photo_dir: PathBuf,
}

impl Database {
    pub fn open(path: PathBuf, photo_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&photo_dir)?;
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA foreign_keys=ON;
             PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS batches(
               id INTEGER PRIMARY KEY,
               batch_no TEXT NOT NULL UNIQUE,
               inspection_date TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS cartons(
               id INTEGER PRIMARY KEY,
               batch_id INTEGER NOT NULL REFERENCES batches(id) ON DELETE CASCADE,
               carton_no TEXT NOT NULL,
               inspector TEXT NOT NULL DEFAULT '',
               reference_qty INTEGER CHECK(reference_qty>0),
               status TEXT NOT NULL DEFAULT 'inspecting',
               UNIQUE(batch_id,carton_no)
             );
             CREATE TABLE IF NOT EXISTS carton_products(
               id INTEGER PRIMARY KEY,
               carton_id INTEGER NOT NULL REFERENCES cartons(id) ON DELETE CASCADE,
               upc TEXT NOT NULL,
               item_no TEXT NOT NULL DEFAULT '',
               color_name TEXT NOT NULL DEFAULT '',
               color_no TEXT NOT NULL DEFAULT '',
               description TEXT NOT NULL DEFAULT '',
               size_dimension TEXT NOT NULL DEFAULT '',
               carton_count INTEGER,
               units_per_carton INTEGER NOT NULL CHECK(units_per_carton>0),
               total_units INTEGER,
               source_order INTEGER NOT NULL DEFAULT 0,
               UNIQUE(carton_id,upc)
             );
             CREATE TABLE IF NOT EXISTS inspection_records(
               id INTEGER PRIMARY KEY,
               batch_id INTEGER NOT NULL REFERENCES batches(id) ON DELETE CASCADE,
               carton_id INTEGER NOT NULL REFERENCES cartons(id) ON DELETE CASCADE,
               barcode TEXT NOT NULL,
               grade TEXT NOT NULL CHECK(grade IN('A','B','C','D')),
               quantity INTEGER NOT NULL CHECK(quantity>0),
               exception_reason TEXT NOT NULL DEFAULT ''
             );
             CREATE TABLE IF NOT EXISTS photos(
               id INTEGER PRIMARY KEY,
               record_id INTEGER NOT NULL REFERENCES inspection_records(id) ON DELETE CASCADE,
               photo_order INTEGER NOT NULL CHECK(photo_order BETWEEN 1 AND 3),
               file_path TEXT NOT NULL,
               UNIQUE(record_id,photo_order)
             );
             CREATE INDEX IF NOT EXISTS idx_records_batch ON inspection_records(batch_id);
             CREATE INDEX IF NOT EXISTS idx_records_carton ON inspection_records(carton_id);",
        )?;
        add_column_if_missing(
            &conn,
            "cartons",
            "inspector",
            "TEXT NOT NULL DEFAULT ''",
        )?;
        Ok(Self { conn, photo_dir })
    }

    pub fn list_batches(&self) -> Result<Vec<Batch>> {
        let mut statement =
            self.conn
                .prepare("SELECT id,batch_no,inspection_date FROM batches ORDER BY id DESC")?;
        let rows = statement
            .query_map([], |row| {
                Ok(Batch {
                    id: row.get(0)?,
                    batch_no: row.get(1)?,
                    inspection_date: row.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn create_batch(&mut self, input: BatchInput) -> Result<i64> {
        if input.batch_no.trim().is_empty() {
            bail!("Batch number is required")
        }
        self.conn.execute(
            "INSERT INTO batches(batch_no,inspection_date) VALUES(?,?)",
            params![input.batch_no.trim(), input.inspection_date],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn delete_batch(&mut self, id: i64) -> Result<()> {
        let photo_paths = self.photo_paths_for_batch(id)?;
        let deleted = self.conn.execute("DELETE FROM batches WHERE id=?", [id])?;
        if deleted == 0 {
            bail!("Batch not found")
        }
        remove_photo_files(photo_paths);
        Ok(())
    }

    pub fn create_carton(&mut self, batch_id: i64, carton_no: String) -> Result<i64> {
        if carton_no.trim().is_empty() {
            bail!("Carton number is required")
        }
        self.conn.execute(
            "INSERT INTO cartons(batch_id,carton_no) VALUES(?,?)",
            params![batch_id, carton_no.trim()],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn rename_carton(&mut self, id: i64, carton_no: String) -> Result<()> {
        if carton_no.trim().is_empty() {
            bail!("箱号不能为空")
        }
        let changed = self.conn.execute(
            "UPDATE cartons SET carton_no=? WHERE id=?",
            params![carton_no.trim(), id],
        )?;
        if changed == 0 {
            bail!("未找到该箱")
        }
        Ok(())
    }

    pub fn update_carton_inspector(&mut self, id: i64, inspector: String) -> Result<()> {
        let changed = self.conn.execute(
            "UPDATE cartons SET inspector=? WHERE id=?",
            params![inspector.trim(), id],
        )?;
        if changed == 0 {
            bail!("未找到该箱")
        }
        Ok(())
    }

    pub fn delete_carton(&mut self, id: i64) -> Result<()> {
        let mut statement = self.conn.prepare(
            "SELECT p.file_path FROM photos p
             JOIN inspection_records r ON r.id=p.record_id
             WHERE r.carton_id=?",
        )?;
        let photo_paths = statement
            .query_map([id], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(statement);
        let deleted = self.conn.execute("DELETE FROM cartons WHERE id=?", [id])?;
        if deleted == 0 {
            bail!("未找到该箱")
        }
        remove_photo_files(photo_paths);
        Ok(())
    }

    pub fn list_cartons(&self, batch_id: i64) -> Result<Vec<Carton>> {
        let mut statement = self.conn.prepare(
            "SELECT c.id,c.carton_no,c.inspector,c.reference_qty,
                    COALESCE(SUM(r.quantity),0),
                    COALESCE(SUM(CASE WHEN r.grade='A' THEN r.quantity ELSE 0 END),0),
                    COALESCE(SUM(CASE WHEN r.grade='B' THEN r.quantity ELSE 0 END),0),
                    COALESCE(SUM(CASE WHEN r.grade='C' THEN r.quantity ELSE 0 END),0),
                    COALESCE(SUM(CASE WHEN r.grade='D' THEN r.quantity ELSE 0 END),0),
                    c.status
             FROM cartons c
             LEFT JOIN inspection_records r ON r.carton_id=c.id
             WHERE c.batch_id=?
             GROUP BY c.id
             ORDER BY CAST(c.carton_no AS INTEGER),c.carton_no",
        )?;
        let rows = statement
            .query_map([batch_id], |row| {
                let inspected: i64 = row.get(4)?;
                let grade_d: i64 = row.get(8)?;
                Ok(Carton {
                    id: row.get(0)?,
                    carton_no: row.get(1)?,
                    inspector: row.get(2)?,
                    reference_qty: row.get(3)?,
                    inspected_qty: inspected,
                    grade_a: row.get(5)?,
                    grade_b: row.get(6)?,
                    grade_c: row.get(7)?,
                    grade_d,
                    sealed_qty: inspected - grade_d,
                    status: row.get(9)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn list_carton_products(&self, carton_id: i64) -> Result<Vec<CartonProduct>> {
        let mut statement = self.conn.prepare(
            "SELECT id,upc,item_no,color_name,color_no,description,size_dimension,
                    carton_count,units_per_carton,total_units
             FROM carton_products WHERE carton_id=? ORDER BY source_order,id",
        )?;
        let rows = statement
            .query_map([carton_id], |row| {
                Ok(CartonProduct {
                    id: row.get(0)?,
                    upc: row.get(1)?,
                    item_no: row.get(2)?,
                    color_name: row.get(3)?,
                    color_no: row.get(4)?,
                    description: row.get(5)?,
                    size_dimension: row.get(6)?,
                    carton_count: row.get(7)?,
                    units_per_carton: row.get(8)?,
                    total_units: row.get(9)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn list_records(&self, batch_id: i64, carton_id: Option<i64>) -> Result<Vec<RecordRow>> {
        let sql = if carton_id.is_some() {
            "SELECT r.id,r.carton_id,c.carton_no,r.barcode,r.grade,r.quantity,r.exception_reason
             FROM inspection_records r JOIN cartons c ON c.id=r.carton_id
             WHERE r.batch_id=? AND r.carton_id=? ORDER BY r.id"
        } else {
            "SELECT r.id,r.carton_id,c.carton_no,r.barcode,r.grade,r.quantity,r.exception_reason
             FROM inspection_records r JOIN cartons c ON c.id=r.carton_id
             WHERE r.batch_id=? ORDER BY CAST(c.carton_no AS INTEGER),c.carton_no,r.id"
        };
        let mut statement = self.conn.prepare(sql)?;
        let map = |row: &rusqlite::Row| {
            Ok(RecordRow {
                id: row.get(0)?,
                carton_id: row.get(1)?,
                carton_no: row.get(2)?,
                barcode: row.get(3)?,
                grade: row.get(4)?,
                quantity: row.get(5)?,
                exception_reason: row.get(6)?,
            })
        };
        if let Some(id) = carton_id {
            Ok(statement
                .query_map(params![batch_id, id], map)?
                .collect::<rusqlite::Result<Vec<_>>>()?)
        } else {
            Ok(statement
                .query_map([batch_id], map)?
                .collect::<rusqlite::Result<Vec<_>>>()?)
        }
    }

    pub fn create_record(&mut self, input: RecordInput) -> Result<i64> {
        validate_record(
            &input.barcode,
            &input.grade,
            input.quantity,
            &input.exception_reason,
            input.photos.len(),
        )?;
        let tx = self.conn.transaction()?;
        ensure_carton(&tx, input.batch_id, input.carton_id)?;
        tx.execute(
            "INSERT INTO inspection_records(batch_id,carton_id,barcode,grade,quantity,exception_reason)
             VALUES(?,?,?,?,?,?)",
            params![
                input.batch_id,
                input.carton_id,
                input.barcode.trim(),
                input.grade,
                input.quantity,
                input.exception_reason.trim()
            ],
        )?;
        let record_id = tx.last_insert_rowid();
        save_photos(&tx, &self.photo_dir, record_id, input.photos)?;
        tx.commit()?;
        Ok(record_id)
    }

    pub fn replace_carton_records(&mut self, input: ReplaceCartonRecordsInput) -> Result<()> {
        let tx = self.conn.transaction()?;
        ensure_carton(&tx, input.batch_id, input.carton_id)?;
        let existing_ids = tx
            .prepare("SELECT id FROM inspection_records WHERE carton_id=?")?
            .query_map([input.carton_id], |row| row.get::<_, i64>(0))?
            .collect::<rusqlite::Result<HashSet<_>>>()?;
        let retained_ids = input
            .records
            .iter()
            .filter_map(|record| record.id)
            .collect::<HashSet<_>>();
        if !retained_ids.is_subset(&existing_ids) {
            bail!("Carton changes contain an invalid record")
        }
        for record in &input.records {
            let photo_count = if record.photos.is_empty() {
                if let Some(id) = record.id {
                    tx.query_row(
                        "SELECT COUNT(*) FROM photos WHERE record_id=?",
                        [id],
                        |row| row.get::<_, usize>(0),
                    )?
                } else {
                    0
                }
            } else {
                record.photos.len()
            };
            validate_record(
                &record.barcode,
                &record.grade,
                record.quantity,
                &record.exception_reason,
                photo_count,
            )?;
        }

        let mut files_to_remove = Vec::new();
        for id in existing_ids.difference(&retained_ids) {
            files_to_remove.extend(photo_paths_for_record(&tx, *id)?);
            tx.execute("DELETE FROM inspection_records WHERE id=?", [id])?;
        }
        for record in input.records {
            if let Some(id) = record.id {
                tx.execute(
                    "UPDATE inspection_records
                     SET barcode=?,grade=?,quantity=?,exception_reason=?
                     WHERE id=? AND carton_id=?",
                    params![
                        record.barcode.trim(),
                        record.grade,
                        record.quantity,
                        record.exception_reason.trim(),
                        id,
                        input.carton_id
                    ],
                )?;
                if !record.photos.is_empty() {
                    files_to_remove.extend(photo_paths_for_record(&tx, id)?);
                    tx.execute("DELETE FROM photos WHERE record_id=?", [id])?;
                    save_photos(&tx, &self.photo_dir, id, record.photos)?;
                }
            } else {
                tx.execute(
                    "INSERT INTO inspection_records(batch_id,carton_id,barcode,grade,quantity,exception_reason)
                     VALUES(?,?,?,?,?,?)",
                    params![
                        input.batch_id,
                        input.carton_id,
                        record.barcode.trim(),
                        record.grade,
                        record.quantity,
                        record.exception_reason.trim()
                    ],
                )?;
                save_photos(
                    &tx,
                    &self.photo_dir,
                    tx.last_insert_rowid(),
                    record.photos,
                )?;
            }
        }
        tx.commit()?;
        remove_photo_files(files_to_remove);
        Ok(())
    }

    pub fn complete_carton(&mut self, id: i64) -> Result<()> {
        let quantity: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(quantity),0) FROM inspection_records WHERE carton_id=?",
            [id],
            |row| row.get(0),
        )?;
        if quantity == 0 {
            bail!("This carton has no inspection records")
        }
        self.conn
            .execute("UPDATE cartons SET status='completed' WHERE id=?", [id])?;
        Ok(())
    }

    pub fn reopen_carton(&mut self, id: i64) -> Result<()> {
        self.conn
            .execute("UPDATE cartons SET status='inspecting' WHERE id=?", [id])?;
        Ok(())
    }

    pub fn import_cartons(
        &mut self,
        batch_id: i64,
        rows: Vec<CartonProductImport>,
    ) -> Result<ImportResult> {
        if rows.is_empty() {
            bail!("The import file contains no valid carton data")
        }
        let tx = self.conn.transaction()?;
        let mut created = 0;
        let mut skipped = 0;
        let mut imported_products = 0;
        let mut cartons = BTreeMap::<String, Vec<&CartonProductImport>>::new();
        for row in &rows {
            if row.carton_no.trim().is_empty()
                || row.upc.trim().is_empty()
                || row.units_per_carton < 1
            {
                bail!("Carton number, UPC, and # OF UNITS PER CRTN must be valid")
            }
            cartons
                .entry(row.carton_no.trim().to_string())
                .or_default()
                .push(row);
        }
        for (carton_no, products) in &cartons {
            let existing = tx
                .query_row(
                    "SELECT id FROM cartons WHERE batch_id=? AND carton_no=?",
                    params![batch_id, carton_no],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?;
            if existing.is_some() {
                skipped += 1;
                continue;
            }
            tx.execute(
                "INSERT INTO cartons(batch_id,carton_no) VALUES(?,?)",
                params![batch_id, carton_no],
            )?;
            created += 1;
            let carton_id = tx.last_insert_rowid();
            let mut reference_qty = 0;
            for (source_order, product) in products.iter().enumerate() {
                imported_products += 1;
                reference_qty += product.units_per_carton;
                tx.execute(
                    "INSERT INTO carton_products(
                       carton_id,upc,item_no,color_name,color_no,description,size_dimension,
                       carton_count,units_per_carton,total_units,source_order
                     ) VALUES(?,?,?,?,?,?,?,?,?,?,?)",
                    params![
                        carton_id,
                        product.upc.trim(),
                        product.item_no.trim(),
                        product.color_name.trim(),
                        product.color_no.trim(),
                        product.description.trim(),
                        product.size_dimension.trim(),
                        product.carton_count,
                        product.units_per_carton,
                        product.total_units,
                        source_order as i64
                    ],
                )?;
            }
            tx.execute(
                "UPDATE cartons SET reference_qty=? WHERE id=?",
                params![reference_qty, carton_id],
            )?;
        }
        tx.commit()?;
        Ok(ImportResult {
            imported: created,
            created,
            skipped,
            products: imported_products,
        })
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    pub fn photos_for_record(&self, record_id: i64) -> Result<Vec<PhotoRow>> {
        let mut statement = self
            .conn
            .prepare("SELECT photo_order,file_path FROM photos WHERE record_id=? ORDER BY photo_order")?;
        let rows = statement
            .query_map([record_id], |row| {
                Ok(PhotoRow {
                    photo_order: row.get(0)?,
                    file_path: row.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    fn photo_paths_for_batch(&self, batch_id: i64) -> Result<Vec<String>> {
        let mut statement = self.conn.prepare(
            "SELECT p.file_path FROM photos p
             JOIN inspection_records r ON r.id=p.record_id
             WHERE r.batch_id=?",
        )?;
        let rows = statement
            .query_map([batch_id], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}

fn ensure_carton(tx: &Transaction, batch_id: i64, carton_id: i64) -> Result<()> {
    tx.query_row(
        "SELECT id FROM cartons WHERE id=? AND batch_id=?",
        params![carton_id, batch_id],
        |row| row.get::<_, i64>(0),
    )
    .context("Carton not found")?;
    Ok(())
}

fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<()> {
    let mut statement = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let exists = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .any(|name| name == column);
    if !exists {
        conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
            [],
        )?;
    }
    Ok(())
}

fn validate_record(
    barcode: &str,
    grade: &str,
    quantity: i64,
    exception_reason: &str,
    photo_count: usize,
) -> Result<()> {
    if barcode.trim().is_empty() {
        bail!("Product barcode is required")
    }
    if !matches!(grade, "A" | "B" | "C" | "D") {
        bail!("Invalid grade")
    }
    if quantity < 1 {
        bail!("Quantity must be greater than 0")
    }
    if exception_reason.trim().is_empty() {
        bail!("异常原因为必填项")
    }
    if grade != "A" && photo_count == 0 {
        bail!("B/C/D 级每条记录至少需要 1 张图片")
    }
    if photo_count > 3 {
        bail!("Maximum 3 images per record")
    }
    Ok(())
}

fn save_photos(
    tx: &Transaction,
    photo_dir: &Path,
    record_id: i64,
    photos: Vec<PhotoInput>,
) -> Result<()> {
    if photos.is_empty() {
        return Ok(())
    }
    let record_dir = photo_dir.join(record_id.to_string());
    std::fs::create_dir_all(&record_dir)?;
    for (index, photo) in photos.into_iter().enumerate() {
        let extension = Path::new(&photo.name)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("jpg")
            .to_ascii_lowercase();
        if !matches!(extension.as_str(), "jpg" | "jpeg" | "png") {
            bail!("Only JPG, JPEG, and PNG images are supported")
        }
        let path = record_dir.join(format!(
            "{}_{}.{}",
            index + 1,
            Uuid::new_v4(),
            extension
        ));
        let bytes = base64::engine::general_purpose::STANDARD.decode(photo.data_base64)?;
        std::fs::write(&path, bytes)?;
        tx.execute(
            "INSERT INTO photos(record_id,photo_order,file_path) VALUES(?,?,?)",
            params![record_id, (index + 1) as i64, path.to_string_lossy()],
        )?;
    }
    Ok(())
}

fn photo_paths_for_record(tx: &Transaction, record_id: i64) -> Result<Vec<String>> {
    let mut statement = tx.prepare("SELECT file_path FROM photos WHERE record_id=?")?;
    let rows = statement
        .query_map([record_id], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn remove_photo_files(paths: Vec<String>) {
    for raw_path in paths {
        let path = PathBuf::from(raw_path);
        let _ = std::fs::remove_file(&path);
        if let Some(parent) = path.parent() {
            let _ = std::fs::remove_dir(parent);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Database {
        let root = std::env::temp_dir().join(format!("clothes-qa-test-{}", Uuid::new_v4()));
        Database::open(root.join("test.db"), root.join("photos")).unwrap()
    }

    fn batch(db: &mut Database) -> i64 {
        db.create_batch(BatchInput {
            batch_no: Uuid::new_v4().to_string(),
            inspection_date: "2026-07-16".into(),
        })
        .unwrap()
    }

    fn input(
        batch_id: i64,
        carton_id: i64,
        barcode: &str,
        grade: &str,
        quantity: i64,
    ) -> RecordInput {
        RecordInput {
            batch_id,
            carton_id,
            barcode: barcode.into(),
            grade: grade.into(),
            quantity,
            exception_reason: "测试原因".into(),
            photos: vec![PhotoInput {
                name: "test.png".into(),
                data_base64: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=".into(),
            }],
        }
    }

    fn imported(carton_no: &str, upc: &str, units: i64) -> CartonProductImport {
        CartonProductImport {
            carton_no: carton_no.into(),
            upc: upc.into(),
            item_no: String::new(),
            color_name: String::new(),
            color_no: String::new(),
            description: String::new(),
            size_dimension: String::new(),
            carton_count: None,
            units_per_carton: units,
            total_units: None,
        }
    }

    #[test]
    fn supports_multiple_barcodes_and_quantities() {
        let mut db = setup();
        let batch_id = batch(&mut db);
        let carton_id = db.create_carton(batch_id, "351".into()).unwrap();
        db.create_record(input(batch_id, carton_id, "001", "B", 3))
            .unwrap();
        db.create_record(input(batch_id, carton_id, "002", "D", 1))
            .unwrap();
        let carton = db.list_cartons(batch_id).unwrap().remove(0);
        assert_eq!((carton.grade_b, carton.grade_d, carton.sealed_qty), (3, 1, 3));
    }

    #[test]
    fn updates_carton_inspector() {
        let mut db = setup();
        let batch_id = batch(&mut db);
        let carton_id = db.create_carton(batch_id, "350".into()).unwrap();
        db.update_carton_inspector(carton_id, "  张三  ".into())
            .unwrap();
        let carton = db.list_cartons(batch_id).unwrap().remove(0);
        assert_eq!(carton.inspector, "张三");
    }

    #[test]
    fn grade_a_allows_no_photo_but_other_grades_require_one() {
        let mut db = setup();
        let batch_id = batch(&mut db);
        let carton_id = db.create_carton(batch_id, "352".into()).unwrap();
        let mut grade_a = input(batch_id, carton_id, "A-001", "A", 1);
        grade_a.photos.clear();
        assert!(db.create_record(grade_a).is_ok());
        let mut grade_b = input(batch_id, carton_id, "B-001", "B", 1);
        grade_b.photos.clear();
        assert!(db.create_record(grade_b).is_err());
    }

    #[test]
    fn imports_multiple_upcs_and_sums_reference_quantity() {
        let mut db = setup();
        let batch_id = batch(&mut db);
        db.import_cartons(
            batch_id,
            vec![
                imported("10", "623555783008", 6),
                imported("10", "623555783022", 19),
            ],
        )
        .unwrap();
        let carton = db.list_cartons(batch_id).unwrap().remove(0);
        assert_eq!(carton.reference_qty, Some(25));
        assert_eq!(db.list_carton_products(carton.id).unwrap().len(), 2);
    }

    #[test]
    fn import_skips_existing_cartons_without_overwriting_products() {
        let mut db = setup();
        let batch_id = batch(&mut db);
        db.import_cartons(batch_id, vec![imported("10", "OLD", 6)])
            .unwrap();
        let result = db
            .import_cartons(
                batch_id,
                vec![imported("10", "NEW", 99), imported("11", "FRESH", 4)],
            )
            .unwrap();
        assert_eq!((result.imported, result.skipped, result.products), (1, 1, 1));
        let cartons = db.list_cartons(batch_id).unwrap();
        let existing = cartons.iter().find(|carton| carton.carton_no == "10").unwrap();
        let products = db.list_carton_products(existing.id).unwrap();
        assert_eq!(products.len(), 1);
        assert_eq!(products[0].upc, "OLD");
        assert_eq!(existing.reference_qty, Some(6));
    }

    #[test]
    fn replaces_carton_records_atomically() {
        let mut db = setup();
        let batch_id = batch(&mut db);
        let carton_id = db.create_carton(batch_id, "1".into()).unwrap();
        let id = db
            .create_record(input(batch_id, carton_id, "OLD", "A", 1))
            .unwrap();
        db.replace_carton_records(ReplaceCartonRecordsInput {
            batch_id,
            carton_id,
            records: vec![RecordEditInput {
                id: Some(id),
                barcode: "NEW".into(),
                grade: "C".into(),
                quantity: 2,
                exception_reason: "污渍".into(),
                photos: vec![],
            }],
        })
        .unwrap();
        let records = db.list_records(batch_id, Some(carton_id)).unwrap();
        assert_eq!((records[0].barcode.as_str(), records[0].quantity), ("NEW", 2));
        assert_eq!(records[0].exception_reason, "污渍");
    }

    #[test]
    fn deletes_batch_and_all_rows() {
        let mut db = setup();
        let batch_id = batch(&mut db);
        let carton_id = db.create_carton(batch_id, "1".into()).unwrap();
        db.create_record(input(batch_id, carton_id, "001", "A", 1))
            .unwrap();
        db.delete_batch(batch_id).unwrap();
        assert!(db.list_batches().unwrap().is_empty());
        assert!(db.list_records(batch_id, None).unwrap().is_empty());
    }
}
