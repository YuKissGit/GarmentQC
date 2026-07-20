mod db;
mod export;
mod models;

use db::Database;
use models::*;
use anyhow::{bail, Context};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Manager, State};

type AppDb = Mutex<Database>;

fn result<T>(value: anyhow::Result<T>) -> Result<T, String> {
    value.map_err(|e| e.to_string())
}

#[tauri::command]
fn list_batches(db: State<AppDb>) -> Result<Vec<Batch>, String> {
    result(db.lock().unwrap().list_batches())
}
#[tauri::command]
fn create_batch(db: State<AppDb>, input: BatchInput) -> Result<i64, String> {
    result(db.lock().unwrap().create_batch(input))
}
#[tauri::command]
fn delete_batch(db: State<AppDb>, id: i64) -> Result<(), String> {
    result(db.lock().unwrap().delete_batch(id))
}
#[tauri::command]
fn list_cartons(db: State<AppDb>, batch_id: i64) -> Result<Vec<Carton>, String> {
    result(db.lock().unwrap().list_cartons(batch_id))
}
#[tauri::command]
fn list_carton_products(db: State<AppDb>, carton_id: i64) -> Result<Vec<CartonProduct>, String> {
    result(db.lock().unwrap().list_carton_products(carton_id))
}
#[tauri::command]
fn create_carton(
    db: State<AppDb>,
    batch_id: i64,
    carton_no: String,
) -> Result<i64, String> {
    result(db.lock().unwrap().create_carton(batch_id, carton_no))
}
#[tauri::command]
fn list_records(
    db: State<AppDb>,
    batch_id: i64,
    carton_id: i64,
) -> Result<Vec<RecordRow>, String> {
    result(db.lock().unwrap().list_records(batch_id, Some(carton_id)))
}
#[tauri::command]
fn create_record(db: State<AppDb>, input: RecordInput) -> Result<i64, String> {
    result(db.lock().unwrap().create_record(input))
}
#[tauri::command]
fn replace_carton_records(
    db: State<AppDb>,
    input: ReplaceCartonRecordsInput,
) -> Result<(), String> {
    result(db.lock().unwrap().replace_carton_records(input))
}
#[tauri::command]
fn complete_carton(db: State<AppDb>, id: i64) -> Result<(), String> {
    result(db.lock().unwrap().complete_carton(id))
}
#[tauri::command]
fn reopen_carton(db: State<AppDb>, id: i64) -> Result<(), String> {
    result(db.lock().unwrap().reopen_carton(id))
}
#[tauri::command]
fn import_cartons(db: State<AppDb>, batch_id: i64, path: String) -> Result<ImportResult, String> {
    result((|| {
        let rows = read_carton_import(std::path::Path::new(&path))?;
        db.lock().unwrap().import_cartons(batch_id, rows)
    })())
}
#[tauri::command]
fn export_carton_template(path: String) -> Result<String, String> {
    result(write_carton_import_template(PathBuf::from(path)))
}
#[tauri::command]
fn export_batch(
    app: tauri::AppHandle,
    db: State<AppDb>,
    batch_id: i64,
    output_dir: String,
) -> Result<Vec<String>, String> {
    let resource_dir = app.path().resource_dir().map_err(|e| e.to_string())?;
    result(export::export_batch(
        &db.lock().unwrap(),
        batch_id,
        PathBuf::from(output_dir),
        resource_dir,
    ))
}

fn read_carton_import(path: &std::path::Path) -> anyhow::Result<Vec<CartonProductImport>> {
    let extension = path.extension().and_then(|value| value.to_str()).unwrap_or("").to_ascii_lowercase();
    if extension != "xlsx" {
        bail!("Use an Excel file in p.xlsx format (.xlsx)")
    }
    let book = umya_spreadsheet::reader::xlsx::read(path)?;
    let sheet = book
        .get_sheet_by_name("Sheet1")
        .or_else(|| book.get_sheet(&0))
        .context("The Excel file contains no worksheet")?;
    let required = ["BOX #", "UPC", "# OF UNITS PER CRTN"];
    let mut header_row = None;
    let mut columns = std::collections::HashMap::<String, u32>::new();
    for row in 1..=sheet.get_highest_row().min(30) {
        let mut current = std::collections::HashMap::new();
        for column in 1..=sheet.get_highest_column() {
            let value = normalize_header(&sheet.get_formatted_value((&column, &row)));
            if !value.is_empty() {
                current.insert(value, column);
            }
        }
        if required.iter().all(|name| current.contains_key(*name)) {
            header_row = Some(row);
            columns = current;
            break;
        }
    }
    let header_row =
        header_row.context("Invalid p.xlsx header: BOX #, UPC, and # OF UNITS PER CRTN are required")?;
    let value = |row: u32, name: &str| -> String {
        columns
            .get(name)
            .map(|column| sheet.get_formatted_value((column, &row)).trim().to_string())
            .unwrap_or_default()
    };
    let mut rows = Vec::new();
    let mut current_carton = String::new();
    let mut seen_upcs = std::collections::HashSet::new();
    for row in (header_row + 1)..=sheet.get_highest_row() {
        let carton = value(row, "BOX #");
        if !carton.is_empty() {
            current_carton = carton;
        }
        let upc = value(row, "UPC");
        let units_raw = value(row, "# OF UNITS PER CRTN");
        if upc.is_empty() && units_raw.is_empty() {
            continue;
        }
        if current_carton.is_empty() {
            bail!("UPC on row {row} has no carton number")
        }
        if upc.is_empty() {
            bail!("UPC is empty on row {row}")
        }
        let units_per_carton = parse_optional_integer(&units_raw)
            .with_context(|| format!("Invalid # OF UNITS PER CRTN on row {row}"))?
            .filter(|quantity| *quantity > 0)
            .with_context(|| format!("# OF UNITS PER CRTN on row {row} must be a positive integer"))?;
        if !seen_upcs.insert((current_carton.clone(), upc.clone())) {
            bail!("Duplicate UPC {upc} in carton {current_carton}")
        }
        rows.push(CartonProductImport {
            carton_no: current_carton.clone(),
            upc,
            item_no: value(row, "货号"),
            color_name: value(row, "COLOR NAME"),
            color_no: value(row, "COLOR#"),
            description: value(row, "MERCHANDISE DESCRIPTION"),
            size_dimension: value(row, "SIZE OR DIMENSION"),
            carton_count: parse_optional_integer(&value(row, "# OF CRTN"))
                .with_context(|| format!("Invalid # OF CRTN on row {row}"))?,
            units_per_carton,
            total_units: parse_optional_integer(&value(row, "TOTAL UNITS"))
                .with_context(|| format!("Invalid TOTAL UNITS on row {row}"))?,
        });
    }
    Ok(rows)
}

fn normalize_header(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('\u{feff}')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_uppercase()
}

fn parse_optional_integer(value: &str) -> anyhow::Result<Option<i64>> {
    let value = value.trim().replace(',', "");
    if value.is_empty() {
        return Ok(None);
    }
    if let Ok(integer) = value.parse::<i64>() {
        return Ok(Some(integer));
    }
    let float = value.parse::<f64>()?;
    if float.fract() != 0.0 {
        bail!("Value is not an integer")
    }
    Ok(Some(float as i64))
}

fn write_carton_import_template(mut path: PathBuf) -> anyhow::Result<String> {
    if path.extension().and_then(|value| value.to_str()).map(|value| value.to_ascii_lowercase()) != Some("xlsx".into()) {
        path.set_extension("xlsx");
    }
    let mut book = umya_spreadsheet::new_file();
    let sheet = book
        .get_sheet_mut(&0)
        .context("Unable to create the template worksheet")?;
    sheet.set_name("Sheet1");
    let headers = ["BOX #","UPC","货号","COLOR NAME","COLOR#","MERCHANDISE DESCRIPTION","SIZE or DIMENSION","# OF CRTN","# OF UNITS PER CRTN","TOTAL UNITS"];
    for (index, header) in headers.iter().enumerate() {
        sheet.get_cell_mut(((index + 1) as u32, 1u32)).set_value(*header);
    }
    for coordinate in ["A1","B1","C1","D1","E1","F1","G1","H1","I1","J1"] {
        let style = sheet.get_cell_mut(coordinate).get_style_mut();
        style.get_font_mut().set_bold(true);
        style.set_background_color("DFF4E7");
    }
    for (column, width) in [("A",12.0),("B",18.0),("C",16.0),("D",20.0),("E",12.0),("F",34.0),("G",22.0),("H",12.0),("I",24.0),("J",14.0)] {
        sheet.get_column_dimension_mut(column).set_width(width);
    }
    sheet.get_row_dimension_mut(&1).set_height(24.0);
    for row in 2..=500 {
        for column in ["A", "B", "C", "E"] {
            sheet.get_cell_mut(format!("{column}{row}")).get_style_mut()
                .get_number_format_mut()
                .set_format_code(umya_spreadsheet::NumberingFormat::FORMAT_TEXT);
        }
    }
    umya_spreadsheet::writer::xlsx::write(&book, &path)?;
    Ok(path.to_string_lossy().to_string())
}

#[cfg(test)]
mod template_tests {
    use super::*;

    #[test]
    fn carton_import_template_has_expected_headers_and_text_column() {
        let path = std::env::temp_dir().join(format!(
            "carton-import-template-{}.xlsx",
            uuid::Uuid::new_v4()
        ));
        write_carton_import_template(path.clone()).unwrap();
        let book = umya_spreadsheet::reader::xlsx::read(&path).unwrap();
        let sheet = book.get_sheet_by_name("Sheet1").unwrap();
        assert_eq!(sheet.get_value("A1"), "BOX #");
        assert_eq!(sheet.get_value("B1"), "UPC");
        assert_eq!(sheet.get_value("I1"), "# OF UNITS PER CRTN");
        assert_eq!(
            sheet
                .get_cell("A2")
                .unwrap()
                .get_style()
                .get_number_format()
                .unwrap()
                .get_format_code(),
            umya_spreadsheet::NumberingFormat::FORMAT_TEXT
        );
    }

    #[test]
    fn reads_p_xlsx_and_carries_box_number_to_following_upc_rows() {
        let path = std::env::temp_dir().join(format!(
            "carton-import-sample-{}.xlsx",
            uuid::Uuid::new_v4()
        ));
        write_carton_import_template(path.clone()).unwrap();
        let mut book = umya_spreadsheet::reader::xlsx::read(&path).unwrap();
        let sheet = book.get_sheet_by_name_mut("Sheet1").unwrap();
        sheet.get_cell_mut("A2").set_value_string("10");
        sheet.get_cell_mut("B2").set_value_string("623555783008");
        sheet.get_cell_mut("I2").set_value_number(6);
        sheet.get_cell_mut("B3").set_value_string("623555783022");
        sheet.get_cell_mut("I3").set_value_number(19);
        umya_spreadsheet::writer::xlsx::write(&book, &path).unwrap();
        let rows = read_carton_import(&path).unwrap();
        let box_ten = rows
            .iter()
            .filter(|row| row.carton_no == "10")
            .collect::<Vec<_>>();
        assert!(box_ten.len() >= 2);
        assert_eq!(box_ten[0].upc, "623555783008");
        assert_eq!(box_ten[0].units_per_carton, 6);
        assert_eq!(box_ten[1].upc, "623555783022");
        assert_eq!(box_ten[1].units_per_carton, 19);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let database =
                Database::open(data_dir.join("quality_v2.db"), data_dir.join("photos_v2"))?;
            app.manage(Mutex::new(database));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_batches,
            create_batch,
            delete_batch,
            list_cartons,
            list_carton_products,
            create_carton,
            list_records,
            create_record,
            replace_carton_records,
            complete_carton,
            reopen_carton,
            import_cartons,
            export_carton_template,
            export_batch
        ])
        .run(tauri::generate_context!())
        .expect("failed to run clothes QA application");
}
