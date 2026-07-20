export type Grade = "A" | "B" | "C" | "D";

export interface Batch {
  id: number;
  batchNo: string;
  inspectionDate: string;
}

export interface Carton {
  id: number;
  cartonNo: string;
  referenceQty: number | null;
  inspectedQty: number;
  gradeA: number;
  gradeB: number;
  gradeC: number;
  gradeD: number;
  sealedQty: number;
  status: string;
}

export interface ImportResult {
  imported: number;
  created: number;
  updated: number;
  products: number;
}

export interface CartonProduct {
  id: number;
  upc: string;
  itemNo: string;
  colorName: string;
  colorNo: string;
  description: string;
  sizeDimension: string;
  cartonCount: number | null;
  unitsPerCarton: number;
  totalUnits: number | null;
}

export interface RecordRow {
  id: number;
  cartonId: number;
  cartonNo: string;
  barcode: string;
  grade: Grade;
  quantity: number;
  exceptionReason: string;
}

export interface PhotoInput {
  name: string;
  dataBase64: string;
}

export interface RecordInput {
  batchId: number;
  cartonId: number;
  barcode: string;
  grade: Grade;
  quantity: number;
  exceptionReason: string;
  photos: PhotoInput[];
}

export interface RecordEditInput {
  id?: number;
  barcode: string;
  grade: Grade;
  quantity: number;
  exceptionReason: string;
  photos: PhotoInput[];
}

export interface ReplaceCartonRecordsInput {
  batchId: number;
  cartonId: number;
  records: RecordEditInput[];
}
