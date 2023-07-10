#[macro_use]
extern crate pest_derive;

use std::cell::RefCell;
use std::collections::HashMap;
use crate::expression::Expression;
use crate::parser::{LabelsMap, parse, Sheet};

mod expression;
mod parser;

struct Spreadsheet {
    rows: Sheet,
    labels_map: LabelsMap,
    evaluating_row: RefCell<usize>,
    evaluating_column: RefCell<usize>,
}

impl Spreadsheet {
    pub fn from_str(input: &str) -> Self {
        let (rows, labels_map) = parse(input.trim()).unwrap();
        Self { rows, labels_map, evaluating_row: RefCell::new(0), evaluating_column: RefCell::new(0) }
    }

    pub fn evaluate(self) -> EvaluatedSpreadsheet {
        let mut columns_length: HashMap<usize, usize> = HashMap::new();

        let result = self.rows
            .iter()
            .map(
                |row| {
                    self.evaluating_row.replace_with(|&mut row_number| row_number + 1);
                    self.evaluating_column.replace(0);
                    row
                        .iter()
                        .enumerate()
                        .map(|(column_index, cell)| {
                            self.evaluating_column.replace_with(|&mut column_number| column_number + 1);
                            let value = cell.evaluate_recursively(&self).to_string();

                            let column_length = columns_length.entry(column_index).or_default();
                            if value.len() > *column_length {
                                *column_length = value.len()
                            }

                            value
                        })
                        .collect::<Vec<String>>()
                }
            ).collect::<Vec<Vec<String>>>();

        EvaluatedSpreadsheet { spreadsheet: result, columns_length }
    }

    pub fn to_string(self) -> String {
        self.evaluate().to_string()
    }

    pub(crate) fn get_cell(&self, row_number: usize, column_number: usize) -> Expression {
        self.rows
            .get(row_number - 1).expect(format!("referencing unknown row {}", row_number).as_str())
            .get(column_number - 1).expect(format!("referencing unknown column {}", column_number).as_str())
            .clone()
    }
}

struct EvaluatedSpreadsheet {
    spreadsheet: Vec<Vec<String>>,
    columns_length: HashMap<usize, usize>,
}

impl EvaluatedSpreadsheet {
    pub fn to_string(self) -> String {
        self.spreadsheet
            .iter()
            .map(
                |row| row
                    .iter()
                    .enumerate()
                    .map(|(column, cell)| format!("{:indent$}", cell, indent = self.columns_length.get(&column).unwrap()))
                    .collect::<Vec<String>>()
                    .join(" | ")
                    .to_string()
            )
            .collect::<Vec<String>>()
            .join("\n")
            .to_string()
    }
}

pub fn column_name_from_index(column: usize) -> String {
    let mut column_name = String::new();
    let mut column = column;

    while column > 0 {
        let char_val = (column - 1) % 26;
        let char = char::from_u32('A' as u32 + char_val as u32).unwrap();

        column_name.insert(0, char);
        column = (column - char_val) / 26;
    }

    return format!("{}", column_name);
}

pub fn column_index_from_name(column: &str) -> usize {
    let mut index = 0;
    let mut mul = 1;

    for c in column.chars().rev() {
        index += (c as usize - 'A' as usize + 1) * mul;
        mul *= 26;
    }

    return index;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_str() {
        let input = r###"
!date|!transaction_id|!tokens|!token_prices|!total_cost
2022-02-20|=concat("t_", text(incFrom(1)))|btc,eth,dai|38341.88,2643.77,1.0003|=sum(spread(split(D2, ",")))
2022-02-21|=^^|bch,eth,dai|304.38,2621.15,1.0001|=E^+sum(spread(split(D3, ",")))
2022-02-22|=^^|sol,eth,dai|85,2604.17,0.9997|=^^
!fee|!cost_threshold
0.09|10000
!adjusted_cost|
=D^v+(D^v*A10)|
!cost_too_high|
1|
=text(bte(@adjusted_cost<1>, @cost_threshold<1>))
"###;
//         let input = r###"
//         abc|12|a
//         aa|=A2|=sum(split("1,1,3", ","))
//         =B2|1|!c
//         a|b|d
//         1|=A^|=@c<1>
//         =^^|5|6
//         =C^v|7|8
//         "###;

        let rows = Spreadsheet::from_str(input);
        let evaluated_spreadsheet = rows.to_string();

        eprintln!("{}", evaluated_spreadsheet);

        // assert_eq!(parsed[0][0], "date".to_owned());
        // assert_eq!(parsed[0][1], "transaction_id".to_owned());
        // assert_eq!(parsed[0][2], "tokens".to_owned());
        // assert_eq!(parsed[0][3], "token_prices".to_owned());
        // assert_eq!(parsed[0][4], "total_cost".to_owned());
        //
        // assert_eq!(parsed[1][0], "2022-02-20".to_owned());
        // assert_eq!(parsed[1][1], "t_1".to_owned());
        // assert_eq!(parsed[1][2], "btc,eth,dai".to_owned());
        // assert_eq!(
        //     parsed[1][3],
        //     "38341.88,2643.77,1.0003".to_owned()
        // );
        // assert_eq!(parsed[1][4], "40985.4581".to_owned());
        //
        // assert_eq!(parsed[2][0], "2022-02-21".to_owned());
        // assert_eq!(parsed[2][1], "t_2".to_owned());
    }
}
