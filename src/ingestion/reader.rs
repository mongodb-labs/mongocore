use std::path::Path;

use polars::prelude::*;

use crate::error::MongoCoreError;
use crate::ingestion::types::{CsvOptions, FileFormat};

/// Detect file format from the file extension.
pub fn detect_format(path: &Path) -> Result<FileFormat, MongoCoreError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "csv" | "tsv" => Ok(FileFormat::Csv),
        "json" => Ok(FileFormat::Json),
        "ndjson" | "jsonl" => Ok(FileFormat::NdJson),
        "parquet" | "pq" => Ok(FileFormat::Parquet),
        other => Err(MongoCoreError::IngestionError(format!(
            "Unknown file extension: '{other}'"
        ))),
    }
}

/// Create a LazyFrame from a file with the given format and CSV options.
pub fn read_lazy(
    path: &Path,
    format: FileFormat,
    csv_options: &CsvOptions,
) -> Result<LazyFrame, MongoCoreError> {
    let format = if format == FileFormat::Auto {
        detect_format(path)?
    } else {
        format
    };

    match format {
        FileFormat::Csv => {
            let mut reader = LazyCsvReader::new(path);

            if let Some(delim) = csv_options.delimiter {
                reader = reader.with_separator(delim);
            }
            if let Some(quote) = csv_options.quote_char {
                reader = reader.with_quote_char(Some(quote));
            }
            if let Some(has_header) = csv_options.has_header {
                reader = reader.with_has_header(has_header);
            }
            if let Some(comment) = csv_options.comment_char {
                let prefix = String::from(comment as char);
                reader = reader.with_comment_prefix(Some(prefix.into()));
            }

            reader
                .finish()
                .map_err(|e| MongoCoreError::IngestionError(e.to_string()))
        }
        FileFormat::Json | FileFormat::NdJson => LazyJsonLineReader::new(path)
            .finish()
            .map_err(|e| MongoCoreError::IngestionError(e.to_string())),
        FileFormat::Parquet => LazyFrame::scan_parquet(path, Default::default())
            .map_err(|e| MongoCoreError::IngestionError(e.to_string())),
        FileFormat::Auto => unreachable!(),
    }
}

/// Count the total number of rows in a file.
pub fn count_rows(
    path: &Path,
    format: FileFormat,
    csv_options: &CsvOptions,
) -> Result<u64, MongoCoreError> {
    let lf = read_lazy(path, format, csv_options)?;
    let df = lf
        .select([len()])
        .collect()
        .map_err(|e| MongoCoreError::IngestionError(e.to_string()))?;

    let count = df
        .column("len")
        .map_err(|e| MongoCoreError::IngestionError(e.to_string()))?
        .u32()
        .map_err(|e| MongoCoreError::IngestionError(e.to_string()))?
        .get(0)
        .unwrap_or(0) as u64;

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_detect_format_csv() {
        let path = Path::new("data.csv");
        assert_eq!(detect_format(path).unwrap(), FileFormat::Csv);
    }

    #[test]
    fn test_detect_format_tsv() {
        let path = Path::new("data.tsv");
        assert_eq!(detect_format(path).unwrap(), FileFormat::Csv);
    }

    #[test]
    fn test_detect_format_json() {
        let path = Path::new("data.json");
        assert_eq!(detect_format(path).unwrap(), FileFormat::Json);
    }

    #[test]
    fn test_detect_format_ndjson() {
        let path = Path::new("data.ndjson");
        assert_eq!(detect_format(path).unwrap(), FileFormat::NdJson);

        let path = Path::new("data.jsonl");
        assert_eq!(detect_format(path).unwrap(), FileFormat::NdJson);
    }

    #[test]
    fn test_detect_format_parquet() {
        let path = Path::new("data.parquet");
        assert_eq!(detect_format(path).unwrap(), FileFormat::Parquet);

        let path = Path::new("data.pq");
        assert_eq!(detect_format(path).unwrap(), FileFormat::Parquet);
    }

    #[test]
    fn test_detect_format_unknown() {
        let path = Path::new("data.xyz");
        assert!(detect_format(path).is_err());
    }

    #[test]
    fn test_read_lazy_csv() {
        let mut file = NamedTempFile::with_suffix(".csv").unwrap();
        writeln!(file, "name,age,city").unwrap();
        writeln!(file, "Alice,30,NYC").unwrap();
        writeln!(file, "Bob,25,LA").unwrap();
        file.flush().unwrap();

        let opts = CsvOptions::default();
        let lf = read_lazy(file.path(), FileFormat::Csv, &opts).unwrap();
        let df = lf.collect().unwrap();

        assert_eq!(df.height(), 2);
        assert_eq!(df.width(), 3);
    }

    #[test]
    fn test_read_lazy_ndjson() {
        let mut file = NamedTempFile::with_suffix(".ndjson").unwrap();
        writeln!(file, r#"{{"name":"Alice","age":30}}"#).unwrap();
        writeln!(file, r#"{{"name":"Bob","age":25}}"#).unwrap();
        file.flush().unwrap();

        let opts = CsvOptions::default();
        let lf = read_lazy(file.path(), FileFormat::NdJson, &opts).unwrap();
        let df = lf.collect().unwrap();

        assert_eq!(df.height(), 2);
        assert_eq!(df.width(), 2);
    }

    #[test]
    fn test_count_rows_csv() {
        let mut file = NamedTempFile::with_suffix(".csv").unwrap();
        writeln!(file, "x,y").unwrap();
        writeln!(file, "1,2").unwrap();
        writeln!(file, "3,4").unwrap();
        writeln!(file, "5,6").unwrap();
        file.flush().unwrap();

        let opts = CsvOptions::default();
        let count = count_rows(file.path(), FileFormat::Csv, &opts).unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_auto_format_delegates() {
        let mut file = NamedTempFile::with_suffix(".csv").unwrap();
        writeln!(file, "a,b").unwrap();
        writeln!(file, "1,2").unwrap();
        file.flush().unwrap();

        let opts = CsvOptions::default();
        let lf = read_lazy(file.path(), FileFormat::Auto, &opts).unwrap();
        let df = lf.collect().unwrap();

        assert_eq!(df.height(), 1);
        assert_eq!(df.width(), 2);
    }
}
