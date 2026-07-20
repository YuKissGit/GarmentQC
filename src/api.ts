import { invoke } from "@tauri-apps/api/core";
import type {
  Batch,
  Carton,
  CartonProduct,
  ImportResult,
  RecordInput,
  RecordRow,
  ReplaceCartonRecordsInput
} from "./types";

export const api = {
  listBatches: () => invoke<Batch[]>("list_batches"),
  createBatch: (input: Omit<Batch, "id">) => invoke<number>("create_batch", { input }),
  deleteBatch: (id: number) => invoke<void>("delete_batch", { id }),
  listCartons: (batchId: number) => invoke<Carton[]>("list_cartons", { batchId }),
  listCartonProducts: (cartonId: number) =>
    invoke<CartonProduct[]>("list_carton_products", { cartonId }),
  createCarton: (batchId: number, cartonNo: string) =>
    invoke<number>("create_carton", { batchId, cartonNo }),
  renameCarton: (id: number, cartonNo: string) =>
    invoke<void>("rename_carton", { id, cartonNo }),
  deleteCarton: (id: number) => invoke<void>("delete_carton", { id }),
  importCartons: (batchId: number, path: string) =>
    invoke<ImportResult>("import_cartons", { batchId, path }),
  exportCartonTemplate: (path: string) =>
    invoke<string>("export_carton_template", { path }),
  listRecords: (batchId: number, cartonId: number) =>
    invoke<RecordRow[]>("list_records", { batchId, cartonId }),
  createRecord: (input: RecordInput) => invoke<number>("create_record", { input }),
  replaceCartonRecords: (input: ReplaceCartonRecordsInput) =>
    invoke<void>("replace_carton_records", { input }),
  completeCarton: (id: number) => invoke<void>("complete_carton", { id }),
  reopenCarton: (id: number) => invoke<void>("reopen_carton", { id }),
  exportBatch: (batchId: number, outputDir: string) =>
    invoke<string[]>("export_batch", { batchId, outputDir })
};
