# Clothing Quality Inspection

A fully offline desktop application for Windows and macOS. It requires no network connection or server deployment. Batches, cartons, UPCs, and inspection records are stored in a local SQLite database. Exception images are stored in the application's local data directory.

## Business Rules

- A batch can contain multiple cartons, and a carton can contain multiple UPCs.
- Carton numbers, UPCs, and reference quantities can be imported from an `.xlsx` file using the `p.xlsx` format.
- The carton sidebar displays up to 70 cartons per page.
- Multiple rows can be added under grades A, B, C, and D.
- Each row contains a product barcode, quantity, optional exception reason, and up to three JPG or PNG images.
- The default quantity is 1 for every grade and can be changed.
- Each entry row is stored as an independent inspection record.
- Grade D items are automatically excluded from the packed quantity. Packed quantity equals A + B + C.
- A carton can only be completed after at least one inspection record has been entered.
- Completed cartons can be reopened for editing.
- Carton edits are saved as a single transaction, preventing partially saved changes.
- The batch report shows quantities and percentages for each grade, together with reference-versus-actual differences for every carton.
- Deleting a batch requires two different confirmations and removes its cartons, inspection records, and locally stored images.

## Excel Export

Each export produces two workbooks:

- `封箱单 BatchNumber Date.xlsx`: generated from `template-2.xlsx`. Each carton receives its own worksheet. Additional worksheets are created automatically when the carton count exceeds the number of worksheets in the template. There is no 42-carton limit. Only grades A, B, and C are included, with quantities expanded to one item per row. Grade D is excluded.
- `入库质检报告 BatchNumber Date.xlsx`: generated from `template-1.xlsx`. It retains the A/B/C/D grade worksheets and the `瑕疵照片 Sample` worksheet.

Grade worksheets expand every quantity to one item per row. In the exception-image worksheet, each grade B record occupies one row and retains its entered quantity, while grades C and D are expanded to one item per row. Columns G, H, and I have a width of 50, and data rows have a height of 60. Up to three images are anchored to move and resize with their cells.

## Local Development

Node.js, Rust stable, and the platform-specific Tauri 2 build dependencies are required.

```bash
npm install
rustup default stable
npm run tauri dev
```

Run the frontend build and backend tests:

```bash
npm run build
cargo test --manifest-path src-tauri/Cargo.toml --lib
```

Build an installer:

```bash
npm run tauri build
```

Windows installers must be built on Windows, and macOS packages must be built on macOS. Images are limited to JPG, JPEG, and PNG.

## Local Data

The current version uses the new `quality_v2.db` database and `photos_v2` image directory. It does not read or migrate data from earlier versions. Existing legacy database files are not automatically deleted.
