use crate::db::Database;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use umya_spreadsheet::structs::drawing::spreadsheet::{
    EditAsValues, MarkerType, TwoCellAnchor,
};
use umya_spreadsheet::structs::Image;
use umya_spreadsheet::{reader, writer};

pub fn export_batch(
    db: &Database,
    batch_id: i64,
    output_dir: PathBuf,
    resource_dir: PathBuf,
) -> Result<Vec<String>> {
    fs::create_dir_all(&output_dir)?;
    let seal_template = find_template(&resource_dir, "template-2.xlsx")?;
    let report_template = find_template(&resource_dir, "template-1.xlsx")?;
    let (batch_no, date): (String, String) = db.connection().query_row(
        "SELECT batch_no,inspection_date FROM batches WHERE id=?",
        [batch_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let safe = date.replace('-', "");
    let seal_out = output_dir.join(format!("封箱单 {} {}.xlsx", batch_no, safe));
    let report_out = output_dir.join(format!("入库质检报告 {} {}.xlsx", batch_no, safe));
    export_seal(db, batch_id, &seal_template, &seal_out)?;
    export_report(db, batch_id, &report_template, &report_out)?;
    Ok(vec![
        seal_out.to_string_lossy().to_string(),
        report_out.to_string_lossy().to_string(),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{BatchInput, CartonProductImport, PhotoInput, RecordInput};
    use uuid::Uuid;

    #[test]
    fn exports_current_seal_and_report_mappings() {
        let root = std::env::temp_dir().join(format!("clothes-qa-export-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let mut db = Database::open(root.join("test.db"), root.join("photos")).unwrap();
        let batch_id = db.create_batch(BatchInput {
            batch_no: "EXPORT-TEST".into(),
            inspection_date: "2026-07-16".into(),
        }).unwrap();
        let carton_id = db.create_carton(batch_id, "001".into()).unwrap();
        db.import_cartons(batch_id, vec![CartonProductImport {
            carton_no: "001".into(),
            upc: "SKU001".into(),
            item_no: String::new(),
            color_name: String::new(),
            color_no: String::new(),
            description: String::new(),
            size_dimension: String::new(),
            carton_count: None,
            units_per_carton: 1,
            total_units: None,
        }]).unwrap();
        db.create_record(RecordInput {
            batch_id,
            carton_id,
            barcode: "SKU-001".into(),
            grade: "B".into(),
            quantity: 17,
            exception_reason: "备注内容".into(),
            photos: vec![PhotoInput {
                name: "sample.png".into(),
                data_base64: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=".into(),
            }],
        }).unwrap();
        db.create_record(RecordInput {
            batch_id,
            carton_id,
            barcode: "SKU-D".into(),
            grade: "D".into(),
            quantity: 50,
            exception_reason: "Cannot repair".into(),
            photos: Vec::new(),
        })
        .unwrap();
        let resource_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();
        let files = export_batch(&db, batch_id, root.clone(), resource_dir).unwrap();
        let book = reader::xlsx::read(Path::new(&files[0])).unwrap();
        let sheet = book.get_sheet_by_name("001").unwrap();
        assert_eq!(*sheet.get_row_dimension(&1).unwrap().get_height(), 61.0);
        assert_eq!(*sheet.get_cell("A1").unwrap().get_style().get_font().unwrap().get_size(), 22.0);
        assert!(sheet.get_merge_cells().iter().any(|range| range.get_range() == "A1:G1"));
        assert_eq!(sheet.get_value("G3"), "备注内容");
        assert_eq!(sheet.get_value("H3"), "");
        assert_eq!(sheet.get_value("A20"), "小计Total");
        assert_eq!(*sheet.get_row_dimension(&19).unwrap().get_height(), 24.0);
        assert_eq!(*sheet.get_row_dimension(&20).unwrap().get_height(), 63.0);
        let report = reader::xlsx::read(Path::new(&files[1])).unwrap();
        assert_eq!(report.get_sheet_count(), 5);
        assert!(report.get_sheet_by_name("质检报告").is_none());
        let photos = report.get_sheet_by_name("瑕疵照片 Sample").unwrap();
        assert_eq!(*photos.get_row_dimension(&2).unwrap().get_height(), 60.0);
        assert_eq!(photos.get_value("A2"), "备注内容");
        assert_eq!(photos.get_value("B2"), "B");
        assert_eq!(photos.get_value("C2"), "17");
        assert_eq!(photos.get_value("D2"), "001");
        assert_eq!(photos.get_value("E2"), "SKU-001");
        assert_eq!(photos.get_value("F2"), "N/A");
        assert_eq!(photos.get_value("B52"), "D");
        assert_eq!(
            photos.get_cell("A52").unwrap().get_style(),
            photos.get_cell("A2").unwrap().get_style()
        );
        let grade_b = report.get_sheet_by_name("B 可增值").unwrap();
        assert_eq!(grade_b.get_value("A2"), "");
        assert_eq!(grade_b.get_value("B2"), "001");
        assert_eq!(grade_b.get_value("C2"), "SKU-001");
        assert_eq!(grade_b.get_value("F2"), "备注内容");
        assert_eq!(grade_b.get_value("C18"), "SKU-001");
        let grade_d = report.get_sheet_by_name("D 不可修復").unwrap();
        assert_eq!(grade_d.get_value("C51"), "SKU-D");
        assert_eq!(
            grade_d.get_cell("B51").unwrap().get_style(),
            grade_d.get_cell("B2").unwrap().get_style()
        );
        for column in ["G", "H", "I"] {
            assert_eq!(*photos.get_column_dimension(column).unwrap().get_width(), 50.0);
        }
    }
}

fn find_template(resource_dir: &Path, name: &str) -> Result<PathBuf> {
    let candidates = [
        resource_dir.join(name),
        resource_dir.join("_up_").join(name),
        std::env::current_dir()?.join(name),
    ];
    candidates
        .into_iter()
        .find(|p| p.exists())
        .with_context(|| format!("Export template not found: {name}"))
}

fn export_seal(db: &Database, batch_id: i64, template: &Path, out: &Path) -> Result<()> {
    let mut book = reader::xlsx::read(template)?;
    let cartons = db.list_cartons(batch_id)?;
    let sheet_count = book.get_sheet_count();
    if cartons.len() > sheet_count {
        let template_sheet = book
            .get_sheet(&0)
            .cloned()
            .context("The packing-list template has no worksheet to copy")?;
        for index in sheet_count..cartons.len() {
            let mut sheet = template_sheet.clone();
            sheet.set_name(format!("__AUTO_{}", index + 1));
            book.add_sheet(sheet)
                .map_err(|error| anyhow::anyhow!("Unable to add a packing-list worksheet: {error}"))?;
        }
    }
    let original_names: Vec<String> = book
        .get_sheet_collection()
        .iter()
        .map(|s| s.get_name().to_string())
        .collect();
    for (index, carton) in cartons.iter().enumerate() {
        let old = &original_names[index];
        let sheet = book
            .get_sheet_by_name_mut(old)
            .context("Unable to read the packing-list worksheet")?;
        sheet.set_name(&carton.carton_no);
        let body_styles = ['A', 'B', 'C', 'D', 'E', 'F', 'G']
            .iter()
            .map(|column| sheet.get_cell(format!("{column}3")).unwrap().get_style().clone())
            .collect::<Vec<_>>();
        let total_styles = ['A', 'B', 'C', 'D', 'E', 'F', 'G']
            .iter()
            .map(|column| sheet.get_cell(format!("{column}19")).unwrap().get_style().clone())
            .collect::<Vec<_>>();
        sheet
            .get_merge_cells_mut()
            .retain(|range| range.get_range() == "A1:G1");
        let last_template_row = sheet.get_highest_row().max(34);
        for row in 3..=last_template_row {
            for col in ['A', 'B', 'C', 'D', 'E', 'F', 'G'] {
                sheet.get_cell_mut(format!("{col}{row}")).set_value("");
            }
        }
        let records = db.list_records(batch_id, Some(carton.id))?;
        let kept: Vec<_> = records.iter().filter(|r| r.grade != "D").collect();
        let mut emitted = 0usize;
        for r in &kept {
            for _ in 0..r.quantity {
                emitted += 1;
                let row = emitted + 2;
                sheet
                    .get_row_dimension_mut(&(row as u32))
                    .set_height(24.0)
                    .set_hidden(false);
                for (index, column) in ['A', 'B', 'C', 'D', 'E', 'F', 'G'].iter().enumerate() {
                    sheet
                        .get_cell_mut(format!("{column}{row}"))
                        .set_style(body_styles[index].clone());
                }
                sheet.get_cell_mut(format!("A{row}")).set_value_number(emitted as f64);
                sheet.get_cell_mut(format!("B{row}")).set_value("");
                sheet
                    .get_cell_mut(format!("C{row}"))
                    .set_value_string(&carton.carton_no)
                    .get_style_mut()
                    .get_number_format_mut()
                    .set_format_code(umya_spreadsheet::NumberingFormat::FORMAT_TEXT);
                sheet
                    .get_cell_mut(format!("D{row}"))
                    .set_value_string(&r.barcode)
                    .get_style_mut()
                    .get_number_format_mut()
                    .set_format_code(umya_spreadsheet::NumberingFormat::FORMAT_TEXT);
                sheet.get_cell_mut(format!("E{row}")).set_value_number(1f64);
                sheet.get_cell_mut(format!("F{row}")).set_value(&r.grade);
                sheet
                    .get_cell_mut(format!("G{row}"))
                    .set_value(&r.exception_reason)
                    .get_style_mut()
                    .get_alignment_mut()
                    .set_wrap_text(true);
            }
        }
        let total_row = emitted + 3;
        for (index, column) in ['A', 'B', 'C', 'D', 'E', 'F', 'G'].iter().enumerate() {
            sheet
                .get_cell_mut(format!("{column}{total_row}"))
                .set_style(total_styles[index].clone());
        }
        sheet.add_merge_cells(format!("A{total_row}:B{total_row}"));
        sheet
            .get_cell_mut(format!("A{total_row}"))
            .set_value("小计Total");
        sheet
            .get_cell_mut(format!("E{total_row}"))
            .set_value_number(emitted as f64);
        sheet
            .get_cell_mut(format!("G{total_row}"))
            .set_value(format!(
                "原箱共{}件，取出{}件D",
                carton.reference_qty.unwrap_or(carton.inspected_qty),
                carton.grade_d
            ))
            .get_style_mut()
            .get_alignment_mut()
            .set_wrap_text(true);
        sheet
            .get_row_dimension_mut(&(total_row as u32))
            .set_height(63.0)
            .set_hidden(false);
        for row in (total_row + 1)..=34.max(total_row + 1) {
            sheet.get_row_dimension_mut(&(row as u32)).set_hidden(true);
        }
    }
    for name in original_names.into_iter().skip(cartons.len()) {
        let _ = book.remove_sheet_by_name(&name);
    }
    writer::xlsx::write(&book, out)?;
    Ok(())
}

fn export_report(db: &Database, batch_id: i64, template: &Path, out: &Path) -> Result<()> {
    let mut book = reader::xlsx::read(template)?;
    let _ = book.remove_sheet_by_name("质检报告");
    let records = db.list_records(batch_id, None)?;
    for (name, grade) in [
        ("A 良品", "A"),
        ("B 可增值", "B"),
        ("C 可修復", "C"),
        ("D 不可修復", "D"),
    ] {
        if let Some(s) = book.get_sheet_by_name_mut(name) {
            s.remove_column("H", &2);
            let body_styles = ['A', 'B', 'C', 'D', 'E', 'F']
                .iter()
                .map(|column| {
                    s.get_cell(format!("{column}2"))
                        .map(|cell| cell.get_style().clone())
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>();
            let last_template_row = s.get_highest_row().max(2);
            for row in 2..=last_template_row {
                for col in ['A', 'B', 'C', 'D', 'E', 'F'] {
                    s.get_cell_mut(format!("{col}{row}")).set_value("");
                }
            }
            let mut emitted = 0usize;
            for r in records.iter().filter(|r| r.grade == grade) {
                for _ in 0..r.quantity {
                    emitted += 1;
                    let row = emitted + 1;
                    for (index, column) in ['A', 'B', 'C', 'D', 'E', 'F'].iter().enumerate() {
                        s.get_cell_mut(format!("{column}{row}"))
                            .set_style(body_styles[index].clone());
                    }
                    s.get_cell_mut(format!("A{row}")).set_value("");
                    s.get_cell_mut(format!("B{row}"))
                        .set_value_string(&r.carton_no)
                        .get_style_mut()
                        .get_number_format_mut()
                        .set_format_code(umya_spreadsheet::NumberingFormat::FORMAT_TEXT);
                    s.get_cell_mut(format!("C{row}"))
                        .set_value_string(&r.barcode)
                        .get_style_mut()
                        .get_number_format_mut()
                        .set_format_code(umya_spreadsheet::NumberingFormat::FORMAT_TEXT);
                    s.get_cell_mut(format!("D{row}")).set_value_number(1.0);
                    s.get_cell_mut(format!("E{row}")).set_value(&r.grade);
                    s.get_cell_mut(format!("F{row}")).set_value(&r.exception_reason);
                }
            }
        }
    }
    if let Some(s) = book.get_sheet_by_name_mut("瑕疵照片 Sample") {
        let body_styles = ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J']
            .iter()
            .map(|column| {
                s.get_cell(format!("{column}2"))
                    .map(|cell| cell.get_style().clone())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>();
        let last_template_row = s.get_highest_row().max(2);
        for row in 2..=last_template_row {
            for col in ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J'] {
                s.get_cell_mut(format!("{col}{row}")).set_value("");
            }
        }
        let mut emitted = 0usize;
        for col in ["G", "H", "I"] {
            s.get_column_dimension_mut(col).set_width(50.0);
        }
        for source in records.iter().filter(|record| record.grade != "A") {
            let seal_sequence = if source.quantity > 1 || source.grade == "D" {
                "N/A".to_string()
            } else {
                let previous: i64 = records
                    .iter()
                    .filter(|record| {
                        record.carton_id == source.carton_id
                            && record.grade != "D"
                            && record.id < source.id
                    })
                    .map(|record| record.quantity)
                    .sum();
                (previous + 1).to_string()
            };
            let copies = if source.grade == "B" { 1 } else { source.quantity };
            for _ in 0..copies {
                emitted += 1;
                let row = emitted + 1;
                s.get_row_dimension_mut(&(row as u32)).set_height(60.0);
                for (index, column) in
                    ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J']
                        .iter()
                        .enumerate()
                {
                    s.get_cell_mut(format!("{column}{row}"))
                        .set_style(body_styles[index].clone());
                }
                s.get_cell_mut(format!("A{row}")).set_value(&source.exception_reason);
                s.get_cell_mut(format!("B{row}")).set_value(&source.grade);
                s.get_cell_mut(format!("C{row}")).set_value_number(if source.grade == "B" { source.quantity as f64 } else { 1.0 });
                s.get_cell_mut(format!("D{row}"))
                    .set_value_string(&source.carton_no)
                    .get_style_mut()
                    .get_number_format_mut()
                    .set_format_code(umya_spreadsheet::NumberingFormat::FORMAT_TEXT);
                s.get_cell_mut(format!("E{row}"))
                    .set_value_string(&source.barcode)
                    .get_style_mut()
                    .get_number_format_mut()
                    .set_format_code(umya_spreadsheet::NumberingFormat::FORMAT_TEXT);
                s.get_cell_mut(format!("F{row}"))
                    .set_value(&seal_sequence);
                s.get_cell_mut(format!("J{row}")).set_value_number(source.id as f64);
                let photos = db.photos_for_record(source.id)?;
                for photo in photos {
                    let col = match photo.photo_order { 1 => 'G', 2 => 'H', _ => 'I' };
                    let mut from_marker = MarkerType::default();
                    from_marker.set_coordinate(format!("{col}{row}"));
                    let mut image = Image::default();
                    image.new_image(&photo.file_path, from_marker.clone());
                    let picture = image
                        .get_one_cell_anchor()
                        .and_then(|anchor| anchor.get_picture())
                        .cloned()
                        .context("Unable to create an in-cell image")?;
                    let mut to_marker = MarkerType::default();
                    let next_col =
                        char::from_u32(col as u32 + 1).context("Invalid image column")?;
                    to_marker.set_coordinate(format!("{next_col}{}", row + 1));
                    let mut anchor = TwoCellAnchor::default();
                    anchor
                        .set_edit_as(EditAsValues::TwoCell)
                        .set_from_marker(from_marker)
                        .set_to_marker(to_marker)
                        .set_picture(picture);
                    image.remove_one_cell_anchor().set_two_cell_anchor(anchor);
                    s.add_image(image);
                }
            }
        }
    }
    // 删除“质检报告”后，模板原有打印区域仍保留旧的 localSheetId，
    // 且其公式本身包含 #REF!。这些失效定义会触发 Excel 的文件修复提示。
    book.get_defined_names_mut().clear();
    for sheet in book.get_sheet_collection_mut() {
        sheet.get_defined_names_mut().clear();
    }
    writer::xlsx::write(&book, out)?;
    Ok(())
}
