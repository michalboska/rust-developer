use std::cmp::max;
use std::error::Error;
use std::fmt::{Display, Formatter};

use csv;
use pad::{Alignment, PadStr};

// could have probably used something like https://crates.io/crates/csv_to_table but it wouldn't be as challenging :)

const COL_PADDING: usize = 2;
const PRINT_ROW_SEPARATORS: bool = false;

pub struct CsvTable {
    columns: Vec<CsvColumn>,
    rows: Vec<Vec<String>>,
}

impl Display for CsvTable {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let spacing_row = self.get_header_or_spacing_row(false);
        f.write_str(&spacing_row)?;
        f.write_str(&self.get_header_or_spacing_row(true))?;
        f.write_str(&spacing_row)?;
        for i in 0..self.rows.len() {
            f.write_str(&self.get_body_row(&self.rows[i]))?;
            if PRINT_ROW_SEPARATORS {
                f.write_str(&spacing_row)?;
            }
        }
        if !PRINT_ROW_SEPARATORS {
            f.write_str(&spacing_row)?;
        }
        return Ok(());
    }
}

struct CsvColumn {
    title: String,
    max_length: usize,
}

impl CsvTable {
    pub fn from_string(input: &str) -> Result<CsvTable, Box<dyn Error>> {
        let mut result = CsvTable {
            columns: Vec::new(),
            rows: Vec::new(),
        };
        let mut reader = csv::ReaderBuilder::new()
            .from_reader(input.as_bytes());
        for header in reader.headers()? {
            CsvTable::process_header_value(header.trim(), &mut result);
        }
        for record_result in reader.records() {
            let mut row = Vec::new();
            let record = record_result?;
            for i in 0..record.len() {
                CsvTable::process_body_value(i, record[i].trim(), &mut row, &mut result);
            };
            result.rows.push(row);
        }
        return Ok(result);
    }

    fn process_header_value(value: &str, csv_struct: &mut CsvTable) {
        csv_struct.columns.push(CsvColumn {
            title: value.to_string(),
            max_length: value.len() + COL_PADDING,
        });
    }

    fn process_body_value(index: usize, value: &str, row: &mut Vec<String>, csv_struct: &mut CsvTable) {
        let columns = &mut csv_struct.columns;
        if index < columns.len() {
            let column = &mut columns[index];
            column.max_length = max(column.max_length, value.len() + COL_PADDING);
        } else {
            let new_column = CsvColumn {
                max_length: value.len() + COL_PADDING,
                title: String::new(),
            };
            columns.push(new_column);
        }
        row.push(value.to_string());
    }

    fn get_header_or_spacing_row(&self, use_col_titles: bool) -> String {
        let dash = "-";
        let mut result = String::from("|");
        for col in &self.columns {
            if use_col_titles {
                let padded_title = col.title.pad_to_width_with_alignment(col.max_length, Alignment::Middle);
                result.push_str(&padded_title);
            } else {
                result.push_str(&dash.repeat(col.max_length));
            }
            result.push('|');
        }
        result.push('\n');
        return result;
    }

    fn get_body_row(&self, row: &Vec<String>) -> String {
        let mut result = String::from("|");
        for i in 0..row.len() {
            let col = &self.columns[i];
            let padded_value = &row[i].pad_to_width_with_alignment(col.max_length, Alignment::Middle);
            result.push_str(&padded_value);
            result.push('|');
        };
        result.push('\n');
        return result;
    }
}