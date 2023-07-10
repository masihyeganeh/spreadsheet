use std::collections::HashMap;
use pest::{Parser, iterators::Pair};
use crate::column_index_from_name;
use crate::expression::{CellReference, ColumnReference, Expression, LabelReference};

#[derive(Parser)]
#[grammar = "spreadsheet.pest"]
pub struct SpreadsheetParser;

pub(crate) type Sheet = Vec<Vec<Expression>>;

pub(crate) type LabelsMap = HashMap<String, (usize, usize)>;

pub(crate) fn parse(input: &str) -> Result<(Sheet, LabelsMap), pest::error::Error<Rule>> {
    let mut row_number = 0;
    let mut column_number;
    let mut rows: Sheet = vec![];
    let mut labels_map: LabelsMap = HashMap::new();

    let pairs = SpreadsheetParser::parse(Rule::file, input)?;

    for pair in pairs {
        let rule = pair.as_rule();

        match rule {
            Rule::file => {
                for pair in pair.into_inner() {
                    let rule = pair.as_rule();
                    match rule {
                        Rule::row => {
                            column_number = 0;
                            let mut cells = vec![];
                            let mut current_cell = None;
                            for pair in pair.into_inner() {
                                let rule = pair.as_rule();
                                match rule {
                                    Rule::cell => {
                                        current_cell = None;
                                        if let Some(new_cell) = parse_cell(pair) {
                                            current_cell = Some(new_cell)
                                        }
                                    }
                                    Rule::delimiter | Rule::end_of_line => {
                                        let expr = if let Some(content) = &current_cell {
                                            content.clone()
                                        } else {
                                            Expression::Empty
                                        };

                                        if let Expression::Label(label) = &expr {
                                            labels_map.insert(label.to_string(), (row_number, column_number));
                                        }

                                        cells.push(expr);

                                        current_cell = None;
                                        column_number += 1;
                                    }
                                    _ => unreachable!(),
                                }
                            }
                            row_number += 1;
                            rows.push(cells);
                        }
                        _ => unreachable!(),
                    }
                }
            }
            _ => unreachable!()
        }
    }
    return Ok((rows, labels_map));
}

pub(crate) fn parse_cell_from_str(input: &str) -> Option<Expression> {
    let pairs = SpreadsheetParser::parse(Rule::cell, input).unwrap();

    for pair in pairs {
        let rule = pair.as_rule();
        match rule {
            Rule::cell => {
                return parse_cell(pair);
            }
            _ => unreachable!()
        };
    }
    None
}

fn parse_cell(pair: Pair<Rule>) -> Option<Expression> {
    for pair in pair.into_inner() {
        let rule = pair.as_rule();
        return match rule {
            Rule::label => {
                Some(parse_label(pair))
            }
            Rule::equation => {
                Some(parse_inner(pair))
            }
            Rule::any_string => {
                Some(Expression::String(pair.as_str().to_string()))
            }
            _ => None
        };
    }
    return None;
}

fn parse_label(pair: Pair<Rule>) -> Expression {
    for pair in pair.into_inner() {
        let rule = pair.as_rule();
        match rule {
            Rule::identifier => {
                return Expression::Label(pair.as_str().to_string());
            }
            _ => unreachable!()
        }
    }
    unreachable!()
}

fn parse_inner(pair: Pair<Rule>) -> Expression {
    for pair in pair.into_inner() {
        let rule = pair.as_rule();
        match rule {
            Rule::expression => {
                return parse_expression(pair);
            }
            _ => unreachable!()
        }
    }
    unreachable!()
}

fn parse_expression(pair: Pair<Rule>) -> Expression {
    let mut params: Vec<Expression> = vec![];
    let mut op = None;
    for pair in pair.into_inner() {
        let rule = pair.as_rule();
        match rule {
            Rule::function_call => {
                let (function_name, function_params) = parse_function_call(pair);
                params.push(Expression::Function { name: function_name, params: function_params });
            }
            Rule::reference => {
                params.push(parse_reference(pair));
            }
            Rule::paren => {
                params.push(parse_inner(pair));
            }
            Rule::copy_evaluated => {
                params.push(Expression::CopyEvaluated(parse_copy_evaluated(pair)));
            }
            Rule::copy_above => {
                params.push(Expression::CopyAbove);
            }
            Rule::label_reference => {
                let (label, row) = parse_label_reference(pair);
                params.push(Expression::LabelReference(LabelReference { label, n_rows: row }));
            }
            Rule::value => {
                params.push(parse_value(pair));
            }
            Rule::operator => {
                op = Some(parse_operator(pair));
            }
            Rule::expression => {
                params.push(parse_expression(pair));
            }
            _ => unreachable!()
        }
        if op.is_some() && params.len() == 2 {
            let rhs = params.pop().unwrap();
            let lhs = params.pop().unwrap();
            let param = match &op {
                Some(Operator::Plus) => {
                    Expression::Plus { args: vec![lhs, rhs] }
                }
                Some(Operator::Minus) => {
                    Expression::Minus { args: vec![lhs, rhs] }
                }
                Some(Operator::Multiply) => {
                    Expression::Multiply { args: vec![lhs, rhs] }
                }
                Some(Operator::Divide) => {
                    Expression::Divide { args: vec![lhs, rhs] }
                }
                _ => unreachable!()
            };
            params.push(param)
        }
    }

    if let Some(value) = params.pop() {
        return value;
    }

    unreachable!()
}

fn parse_function_call(pair: Pair<Rule>) -> (String, Vec<Expression>) {
    let mut function_name = String::new();
    let mut function_params = vec![];
    for pair in pair.into_inner() {
        let rule = pair.as_rule();
        match rule {
            Rule::identifier => {
                function_name = pair.as_str().to_string();
            }
            Rule::expression => {
                let param = parse_expression(pair);
                function_params.push(param)
            }
            _ => unreachable!()
        }
    }
    return (function_name, function_params);
}

fn parse_reference(pair: Pair<Rule>) -> Expression {
    for pair in pair.into_inner() {
        let rule = pair.as_rule();
        match rule {
            Rule::cell_reference => {
                return Expression::CellReference(parse_cell_reference(pair));
            }
            Rule::column_reference => {
                return Expression::ColumnReference(parse_column_reference(pair));
            }
            _ => unreachable!()
        }
    }
    unreachable!()
}

fn parse_cell_reference(pair: Pair<Rule>) -> CellReference {
    let mut column_name = String::new();
    let mut row_number: usize = 0;
    for pair in pair.clone().into_inner() {
        let rule = pair.as_rule();
        match rule {
            Rule::column => {
                column_name = pair.as_str().to_string();
            }
            Rule::integer => {
                row_number = pair.as_str().parse().expect("row number should be integer");
            }
            _ => unreachable!()
        }
    }

    CellReference {
        name: pair.as_str().to_string(),
        column_name: column_name.to_string(),
        column: column_index_from_name(&column_name),
        row: row_number,
    }
}

fn parse_column_reference(pair: Pair<Rule>) -> ColumnReference {
    for pair in pair.clone().into_inner() {
        let rule = pair.as_rule();
        match rule {
            Rule::column => {
                return ColumnReference {
                    name: pair.as_str().to_string(),
                    column: column_index_from_name(pair.as_str()),
                };
            }
            _ => unreachable!()
        }
    }
    unreachable!()
}

fn parse_copy_evaluated(pair: Pair<Rule>) -> ColumnReference {
    for pair in pair.into_inner() {
        let rule = pair.as_rule();
        match rule {
            Rule::column => {
                let name = pair.as_str().to_string();
                let column = column_index_from_name(pair.as_str());
                return ColumnReference { name, column };
            }
            _ => unreachable!()
        }
    }
    unreachable!()
}

fn parse_label_reference(pair: Pair<Rule>) -> (String, usize) {
    let mut label = String::new();
    let mut row = 0;
    for pair in pair.into_inner() {
        let rule = pair.as_rule();
        match rule {
            Rule::identifier => {
                label = pair.as_str().to_string();
            }
            Rule::integer => {
                row = pair.as_str().to_string().parse().expect("row number should be an integer");
            }
            _ => unreachable!()
        }
    }
    (label, row)
}

fn parse_value(pair: Pair<Rule>) -> Expression {
    for pair in pair.into_inner() {
        let rule = pair.as_rule();
        match rule {
            Rule::string => {
                for pair in pair.into_inner() {
                    let rule = pair.as_rule();
                    return match rule {
                        Rule::inner => {
                            Expression::String(pair.as_str().to_string())
                        }
                        _ => unreachable!()
                    };
                }
            }
            Rule::float | Rule::integer => {
                return Expression::Number(pair.as_str().to_string().parse().expect("expected number"));
            }
            _ => unreachable!()
        }
    }
    unreachable!()
}

#[derive(Debug)]
enum Operator {
    Plus,
    Minus,
    Multiply,
    Divide,
}

fn parse_operator(pair: Pair<Rule>) -> Operator {
    for pair in pair.into_inner() {
        let rule = pair.as_rule();
        match rule {
            Rule::plus => return Operator::Plus,
            Rule::minus => return Operator::Minus,
            Rule::multiply => return Operator::Multiply,
            Rule::divide => return Operator::Divide,
            _ => unreachable!()
        }
    }
    unreachable!()
}
