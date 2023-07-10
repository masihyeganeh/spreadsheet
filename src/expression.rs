use crate::parser::parse_cell_from_str;
use crate::Spreadsheet;

const RECURSION_LIMIT: usize = 256;

#[derive(Debug, Clone, PartialEq)]
pub struct CellReference {
    pub name: String,
    pub column_name: String,
    pub column: usize,
    pub row: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LabelReference {
    pub label: String,
    pub n_rows: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColumnReference {
    pub name: String,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Expression {
    Empty,
    Number(f64),
    Label(String),
    String(String),
    List { expressions: Vec<Expression> },
    Spread(Vec<Expression>),
    CellReference(CellReference),
    LabelReference(LabelReference),
    ColumnReference(ColumnReference),
    CopyAbove,
    CopyEvaluated(ColumnReference),
    Function { name: String, params: Vec<Expression> },
    Plus { args: Vec<Expression> },
    Minus { args: Vec<Expression> },
    Multiply { args: Vec<Expression> },
    Divide { args: Vec<Expression> },
}

impl Expression {
    pub(crate) fn evaluate_recursively(&self, spreadsheet: &Spreadsheet) -> Expression {
        let mut expr = self.clone();
        for _ in 0..RECURSION_LIMIT {
            if matches!(expr, Expression::String(_)) {
                return expr.clone();
            }
            expr = expr.evaluate(spreadsheet);
        }
        eprintln!("{:?}", expr);
        panic!("recursion limit reached")
    }

    pub(crate) fn evaluate(&self, spreadsheet: &Spreadsheet) -> Expression {
        match self {
            Expression::Empty => Expression::String(String::new()),
            Expression::Number(number) => Expression::String(number.to_string()),
            Expression::String(string) => Expression::String(string.clone()),
            Expression::Label(name) => Expression::String(name.to_string()),
            Expression::CellReference(cell_ref) => spreadsheet.get_cell(cell_ref.row, cell_ref.column),
            Expression::LabelReference(label_ref) => {
                if let Some((label_row_number, label_column_number)) = spreadsheet.labels_map.get(&label_ref.label) {
                    return spreadsheet.get_cell(label_row_number + label_ref.n_rows + 1, label_column_number + 1).evaluate(spreadsheet);
                }
                Expression::String("error".to_string())
            }
            Expression::CopyAbove => {
                spreadsheet.evaluating_row.replace_with(|&mut row_number| row_number - 1);
                let above_cell = spreadsheet.get_cell(spreadsheet.evaluating_row.borrow().clone(), spreadsheet.evaluating_column.borrow().clone());
                if matches!(above_cell, Expression::CopyAbove) {
                    if let Expression::CellReference(cell_ref) = above_cell.evaluate(spreadsheet) {
                        return Expression::CellReference(CellReference {
                            name: format!("{}{}", cell_ref.column_name, cell_ref.row).to_string(),
                            column_name: cell_ref.column_name.to_string(),
                            column: cell_ref.column,
                            row: cell_ref.row - 1,
                        });
                    }
                    unreachable!()
                }
                above_cell
            }
            Expression::CopyEvaluated(column_ref) => spreadsheet.get_cell(spreadsheet.evaluating_row.borrow().clone() - 1, column_ref.column).evaluate(spreadsheet),
            Expression::ColumnReference(column_ref) => {
                for row in spreadsheet.rows.iter().rev() {
                    if let Some(cell) = row.get(column_ref.column - 1) {
                        match cell {
                            Expression::Empty | Expression::Label(_) => {}
                            expr => return expr.evaluate(spreadsheet)
                        }
                    }
                }
                Expression::String("error".to_string())
            }
            Expression::Plus { args } => Expression::Number(args.iter().fold(0.0, |acc, cur| acc + cur.evaluate(spreadsheet).to_number())),
            Expression::Minus { args } => {
                let first = args[0].evaluate(spreadsheet).to_number();
                Expression::Number(args[1..].iter().fold(first, |acc, cur| acc + cur.evaluate(spreadsheet).to_number()))
            }
            Expression::Multiply { args } => Expression::Number(args.iter().fold(1.0, |acc, cur| acc * cur.evaluate(spreadsheet).to_number())),
            Expression::Divide { args } => {
                let first = args[0].evaluate(spreadsheet).to_number();
                Expression::Number(args[1..].iter().fold(first, |acc, cur| {
                    let value = cur.evaluate(spreadsheet).to_number();
                    if value == 0.0 {
                        panic!("division by zero");
                    }
                    acc / value
                }))
            }
            Expression::Function { name, params } => {
                let params: Vec<Expression> = params
                    .iter()
                    .flat_map(|expr| match expr.evaluate(spreadsheet) {
                        Expression::Spread(ref expressions) => expressions.clone(),
                        expr => vec![expr.clone()]
                    }).collect();

                match name.to_lowercase().as_str() {
                    "sum" => Expression::Number(params.iter().fold(0.0, |acc, cur| acc + cur.evaluate(spreadsheet).to_number())
                    ),
                    "gte" | "bte" => {
                        if params.len() != 2 {
                            panic!("binary operation needs 2 params")
                        }
                        Expression::String((params[0].evaluate(spreadsheet).to_number() >= params[1].evaluate(spreadsheet).to_number()).to_string())
                    }
                    "lte" => {
                        if params.len() != 2 {
                            panic!("binary operation needs 2 params")
                        }
                        Expression::String((params[0].evaluate(spreadsheet).to_number() <= params[1].evaluate(spreadsheet).to_number()).to_string())
                    }
                    "text" => Expression::String(params[0].evaluate(spreadsheet).to_string()),
                    "split" => {
                        if params.len() != 2 {
                            panic!("binary operation needs 2 params")
                        }
                        let text = params[0].evaluate(spreadsheet).to_string();
                        let delim = params[1].evaluate(spreadsheet).to_string();
                        let list = text.split(&delim).map(|input| {
                            parse_cell_from_str(input).unwrap_or(Expression::String(input.to_string()))
                        }).collect::<Vec<Expression>>();
                        Expression::List { expressions: list }
                    }
                    "concat" => Expression::String(params.iter().fold(String::new(), |mut acc, cur| {
                        acc.push_str(&cur.evaluate(spreadsheet).to_string());
                        acc
                    })),
                    "spread" => Expression::Spread(match params[0].evaluate(spreadsheet) {
                        Expression::List { expressions } => expressions.clone(),
                        _ => panic!("spread only works on lists")
                    }),
                    "incfrom" => Expression::Number(params[0].evaluate(spreadsheet).to_number()),
                    function_name => panic!("unknown function '{}'", function_name),
                }
            }
            Expression::List { expressions: _ } => self.clone(),
            Expression::Spread(_) => self.clone(),
        }
    }

    fn to_number(&self) -> f64 {
        match self {
            Expression::Number(number) => *number,
            Expression::String(string) => string.parse::<f64>().unwrap_or(0.0),
            Expression::Spread(_) => 0.0,
            _ => panic!("expected number")
        }
    }
}

impl std::fmt::Display for Expression {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let number = match self {
            Expression::Number(number) => *number,
            Expression::String(string) => match string.parse::<f64>() {
                Ok(number) => number,
                Err(_) => return fmt.write_str(string),
            },
            _ => return fmt.write_str("unexpected error")
        };

        if number.fract() == 0.0 {
            fmt.write_str(&format!("{}", number))
        } else {
            fmt.write_str(&format!("{:.2}", number))
        }
    }
}
