//! MySQL `SELECT ... INTO OUTFILE ... FIELDS TERMINATED BY '\t' ESCAPED BY '\\' LINES TERMINATED BY '\n'`
//! text. The first line is a header of column names (the export unions one in). A field is
//! backslash-escaped: `\\` is a backslash, `\t` is NOT used (a tab inside a value is written as
//! backslash followed by a literal tab), a backslash followed by a literal newline is a newline
//! inside the value, `\0` is NUL, and the two characters `\N` as a whole field are SQL NULL.

use anyhow::{bail, Result};
use std::collections::HashMap;

/// A parsed export: column names and rows of optional strings (`None` is SQL NULL).
#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Option<String>>>,
    index: HashMap<String, usize>,
}

/// One row with access by column name.
pub struct Row<'a> {
    table: &'a Table,
    cells: &'a [Option<String>],
}

/// Split text into records (lines) of fields, decoding the backslash escapes; `None` is SQL NULL.
/// A newline or tab preceded by a backslash belongs to the value; every other one ends a field / record.
pub fn records(text: &str) -> Result<Vec<Vec<Option<String>>>> {
    let mut records: Vec<Vec<Option<String>>> = Vec::new();
    let mut row: Vec<Option<String>> = Vec::new();
    let mut field = String::new();
    let mut is_null = false;
    let mut chars = text.chars().peekable();
    let mut field_started = false;
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                field_started = true;
                match chars.next() {
                    Some('N') if field.is_empty() && !is_null => is_null = true,
                    Some('0') => field.push('\0'),
                    Some(other) => field.push(other),
                    None => bail!("tsv: text ends inside an escape"),
                }
            }
            '\t' => {
                row.push(if is_null {
                    None
                } else {
                    Some(std::mem::take(&mut field))
                });
                field.clear();
                is_null = false;
                field_started = false;
            }
            '\n' => {
                row.push(if is_null {
                    None
                } else {
                    Some(std::mem::take(&mut field))
                });
                field.clear();
                is_null = false;
                field_started = false;
                records.push(std::mem::take(&mut row));
            }
            other => {
                field_started = true;
                field.push(other);
            }
        }
    }
    if field_started || !row.is_empty() {
        row.push(if is_null { None } else { Some(field) });
        records.push(row);
    }
    Ok(records)
}

/// The inverse of the decoding: a backslash, tab or newline inside a value gets a backslash before
/// it and NUL is written `\0`, so `records` reads the value back unchanged.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' | '\t' | '\n' => {
                out.push('\\');
                out.push(c);
            }
            '\0' => out.push_str("\\0"),
            _ => out.push(c),
        }
    }
    out
}

impl Table {
    pub fn parse(text: &str) -> Result<Table> {
        let mut it = records(text)?.into_iter();
        let Some(header) = it.next() else {
            bail!("tsv: empty text, no header line");
        };
        let columns: Vec<String> = header.into_iter().map(|c| c.unwrap_or_default()).collect();
        let index = columns
            .iter()
            .enumerate()
            .map(|(i, c)| (c.clone(), i))
            .collect();
        let rows: Vec<_> = it.collect();
        for (n, r) in rows.iter().enumerate() {
            if r.len() != columns.len() {
                bail!(
                    "tsv: row {} has {} fields, header has {}",
                    n + 1,
                    r.len(),
                    columns.len()
                );
            }
        }
        Ok(Table {
            columns,
            rows,
            index,
        })
    }

    pub fn load(path: &std::path::Path) -> Result<Table> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("reading {}: {e}", path.display()))?;
        Table::parse(&text).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))
    }

    pub fn rows(&self) -> impl Iterator<Item = Row<'_>> {
        self.rows.iter().map(|cells| Row { table: self, cells })
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

impl<'a> Row<'a> {
    fn cell(&self, col: &str) -> Result<&'a Option<String>> {
        let i = *self.table.index.get(col).ok_or_else(|| {
            anyhow::anyhow!("tsv: no column `{col}` (have {:?})", self.table.columns)
        })?;
        Ok(&self.cells[i])
    }

    /// The text of a column, NULL read as the empty string.
    pub fn text(&self, col: &str) -> Result<String> {
        Ok(self.cell(col)?.clone().unwrap_or_default())
    }

    pub fn u32(&self, col: &str) -> Result<u32> {
        let t = self.text(col)?;
        if t.is_empty() {
            return Ok(0);
        }
        t.parse()
            .map_err(|e| anyhow::anyhow!("column `{col}` = `{t}`: {e}"))
    }

    pub fn i32(&self, col: &str) -> Result<i32> {
        let t = self.text(col)?;
        if t.is_empty() {
            return Ok(0);
        }
        t.parse()
            .map_err(|e| anyhow::anyhow!("column `{col}` = `{t}`: {e}"))
    }

    pub fn f32(&self, col: &str) -> Result<f32> {
        let t = self.text(col)?;
        if t.is_empty() {
            return Ok(0.0);
        }
        t.parse()
            .map_err(|e| anyhow::anyhow!("column `{col}` = `{t}`: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_header_rows_and_typed_columns() {
        let t = Table::parse("entry\tname\tx\n38\tDefias Thug\t-8913.5\n6\tKobold Vermin\t\\N\n")
            .unwrap();
        assert_eq!(t.columns, vec!["entry", "name", "x"]);
        assert_eq!(t.len(), 2);
        let rows: Vec<_> = t.rows().collect();
        assert_eq!(rows[0].u32("entry").unwrap(), 38);
        assert_eq!(rows[0].text("name").unwrap(), "Defias Thug");
        assert!((rows[0].f32("x").unwrap() + 8913.5).abs() < 1e-3);
        // NULL reads as empty / zero.
        assert_eq!(rows[1].f32("x").unwrap(), 0.0);
    }

    #[test]
    fn escapes_keep_tabs_newlines_and_backslashes_inside_a_value() {
        // A value `a<TAB>b<NL>c\d`: MySQL writes backslash+tab, backslash+newline, double backslash.
        let text = "id\ttext\n1\ta\\\tb\\\nc\\\\d\n2\tplain\n";
        let t = Table::parse(text).unwrap();
        assert_eq!(t.len(), 2);
        let rows: Vec<_> = t.rows().collect();
        assert_eq!(rows[0].text("text").unwrap(), "a\tb\nc\\d");
        assert_eq!(rows[1].text("text").unwrap(), "plain");
    }

    #[test]
    fn backslash_n_is_null_only_as_a_whole_field() {
        let t = Table::parse("a\tb\nx\\N\t\\N\n").unwrap();
        let rows: Vec<_> = t.rows().collect();
        // `x\N` is the text `xN`; `\N` alone is NULL.
        assert_eq!(rows[0].text("a").unwrap(), "xN");
        assert_eq!(t.rows[0][1], None);
    }

    #[test]
    fn a_row_with_the_wrong_field_count_is_an_error() {
        let e = Table::parse("a\tb\n1\n").unwrap_err().to_string();
        assert!(e.contains("row 1 has 1 fields, header has 2"), "{e}");
    }

    #[test]
    fn escape_is_the_inverse_of_the_parser() {
        let values = [
            "",
            "plain",
            "a\tb",
            "line one\nline two",
            "back\\slash",
            "\\N",
            "N",
            "\\",
            "ends with a backslash\\",
            "tab at the end\t",
            "\n",
            "\0 and \\0",
            "carriage return at the end\r",
            "Åäö, \u{2014} and \u{1F43A}",
        ];
        for v in values {
            let text = format!("c\n{}\n", escape(v));
            let t = Table::parse(&text).unwrap();
            assert_eq!(t.len(), 1, "{v:?} -> {text:?}");
            assert_eq!(t.rows[0][0].as_deref(), Some(v), "{v:?} -> {text:?}");
        }
        // Two columns, so the escaped tab and newline must not split the record.
        let text = format!("a\tb\n{}\t{}\n", escape("x\ty"), escape("p\nq"));
        let t = Table::parse(&text).unwrap();
        assert_eq!(t.len(), 1);
        assert_eq!(t.rows[0][0].as_deref(), Some("x\ty"));
        assert_eq!(t.rows[0][1].as_deref(), Some("p\nq"));
    }

    #[test]
    fn escape_writes_exactly_the_documented_sequences() {
        assert_eq!(escape("a\tb\nc\\d\0"), "a\\\tb\\\nc\\\\d\\0");
        assert_eq!(escape("no specials"), "no specials");
    }

    #[test]
    fn records_split_lines_without_a_header_rule() {
        let r = records("one\n\ntwo\tthree\\\tfour\n").unwrap();
        assert_eq!(r.len(), 3);
        assert_eq!(r[0], vec![Some("one".to_string())]);
        assert_eq!(
            r[1],
            vec![Some(String::new())],
            "a blank line is one empty field"
        );
        assert_eq!(
            r[2],
            vec![Some("two".to_string()), Some("three\tfour".to_string())]
        );
        assert!(records("").unwrap().is_empty());
        assert!(records("x\\").unwrap_err().to_string().contains("escape"));
    }

    #[test]
    fn an_unknown_column_names_the_columns_that_exist() {
        let t = Table::parse("a\n1\n").unwrap();
        let e = t.rows().next().unwrap().u32("zzz").unwrap_err().to_string();
        assert!(e.contains("no column `zzz`") && e.contains("\"a\""), "{e}");
    }
}
