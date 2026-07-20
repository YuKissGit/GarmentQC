use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Batch {
    pub id: i64,
    pub batch_no: String,
    pub inspection_date: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchInput {
    pub batch_no: String,
    pub inspection_date: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Carton {
    pub id: i64,
    pub carton_no: String,
    pub reference_qty: Option<i64>,
    pub inspected_qty: i64,
    pub grade_a: i64,
    pub grade_b: i64,
    pub grade_c: i64,
    pub grade_d: i64,
    pub sealed_qty: i64,
    pub status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub imported: i64,
    pub created: i64,
    pub updated: i64,
    pub products: i64,
}

#[derive(Debug, Clone)]
pub struct CartonProductImport {
    pub carton_no: String,
    pub upc: String,
    pub item_no: String,
    pub color_name: String,
    pub color_no: String,
    pub description: String,
    pub size_dimension: String,
    pub carton_count: Option<i64>,
    pub units_per_carton: i64,
    pub total_units: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CartonProduct {
    pub id: i64,
    pub upc: String,
    pub item_no: String,
    pub color_name: String,
    pub color_no: String,
    pub description: String,
    pub size_dimension: String,
    pub carton_count: Option<i64>,
    pub units_per_carton: i64,
    pub total_units: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordRow {
    pub id: i64,
    pub carton_id: i64,
    pub carton_no: String,
    pub barcode: String,
    pub grade: String,
    pub quantity: i64,
    pub exception_reason: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoInput {
    pub name: String,
    pub data_base64: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordInput {
    pub batch_id: i64,
    pub carton_id: i64,
    pub barcode: String,
    pub grade: String,
    pub quantity: i64,
    pub exception_reason: String,
    pub photos: Vec<PhotoInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordEditInput {
    pub id: Option<i64>,
    pub barcode: String,
    pub grade: String,
    pub quantity: i64,
    pub exception_reason: String,
    pub photos: Vec<PhotoInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceCartonRecordsInput {
    pub batch_id: i64,
    pub carton_id: i64,
    pub records: Vec<RecordEditInput>,
}

#[derive(Debug)]
pub struct PhotoRow {
    pub photo_order: i64,
    pub file_path: String,
}
