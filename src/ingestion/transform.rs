use polars::prelude::*;

use crate::error::MongoCoreError;

#[derive(Debug, Clone)]
pub enum TransformOp {
    Rename { from: String, to: String },
    Drop(Vec<String>),
    Filter(String),
    Cast { column: String, dtype: DataType },
    Select(Vec<String>),
    Compute { name: String, expr: String },
}

/// Apply a sequence of string expressions to a LazyFrame.
pub fn apply_expressions(
    mut lf: LazyFrame,
    expressions: &[String],
) -> Result<LazyFrame, MongoCoreError> {
    for expr_str in expressions {
        let op = parse_expression(expr_str)?;
        lf = compile_transform(&op, lf)?;
    }
    Ok(lf)
}

/// Parse a string expression into a TransformOp.
fn parse_expression(expr_str: &str) -> Result<TransformOp, MongoCoreError> {
    let expr_str = expr_str.trim();
    let open = expr_str.find('(').ok_or_else(|| {
        MongoCoreError::IngestionError(format!("Invalid expression, missing '(': {expr_str}"))
    })?;
    if !expr_str.ends_with(')') {
        return Err(MongoCoreError::IngestionError(format!(
            "Invalid expression, missing closing ')': {expr_str}"
        )));
    }
    let func = &expr_str[..open];
    let inner = &expr_str[open + 1..expr_str.len() - 1];

    match func {
        "rename" => {
            let parts: Vec<&str> = inner.splitn(2, ',').map(|s| s.trim()).collect();
            if parts.len() != 2 {
                return Err(MongoCoreError::IngestionError(
                    "rename requires exactly 2 arguments".to_string(),
                ));
            }
            Ok(TransformOp::Rename {
                from: parts[0].to_string(),
                to: parts[1].to_string(),
            })
        }
        "drop" => {
            let cols: Vec<String> = inner.split(',').map(|s| s.trim().to_string()).collect();
            Ok(TransformOp::Drop(cols))
        }
        "filter" => Ok(TransformOp::Filter(inner.trim().to_string())),
        "cast" => {
            let parts: Vec<&str> = inner.splitn(2, ',').map(|s| s.trim()).collect();
            if parts.len() != 2 {
                return Err(MongoCoreError::IngestionError(
                    "cast requires exactly 2 arguments".to_string(),
                ));
            }
            let dtype = parse_polars_dtype(parts[1])?;
            Ok(TransformOp::Cast {
                column: parts[0].to_string(),
                dtype,
            })
        }
        "select" => {
            let cols: Vec<String> = inner.split(',').map(|s| s.trim().to_string()).collect();
            Ok(TransformOp::Select(cols))
        }
        "compute" => {
            let parts: Vec<&str> = inner.splitn(2, ',').map(|s| s.trim()).collect();
            if parts.len() != 2 {
                return Err(MongoCoreError::IngestionError(
                    "compute requires 2 arguments: name, expression (e.g. compute(profit, DomesticGross + ForeignGross - Budget))".to_string(),
                ));
            }
            Ok(TransformOp::Compute {
                name: parts[0].to_string(),
                expr: parts[1].to_string(),
            })
        }
        _ => Err(MongoCoreError::IngestionError(format!(
            "Unknown transform function: {func}"
        ))),
    }
}

/// Apply a TransformOp to a LazyFrame.
fn compile_transform(op: &TransformOp, lf: LazyFrame) -> Result<LazyFrame, MongoCoreError> {
    match op {
        TransformOp::Rename { from, to } => Ok(lf.rename([from.as_str()], [to.as_str()], true)),
        TransformOp::Drop(cols) => {
            let col_refs: Vec<&str> = cols.iter().map(|s| s.as_str()).collect();
            Ok(lf.drop(col_refs))
        }
        TransformOp::Filter(expr_str) => {
            let expr = parse_filter_expr(expr_str)?;
            Ok(lf.filter(expr))
        }
        TransformOp::Cast { column, dtype } => {
            Ok(lf.with_column(col(column.as_str()).cast(dtype.clone())))
        }
        TransformOp::Select(cols) => {
            let exprs: Vec<Expr> = cols.iter().map(|c| col(c.as_str())).collect();
            Ok(lf.select(exprs))
        }
        TransformOp::Compute { name, expr } => {
            let polars_expr = parse_arithmetic_expr(expr)?;
            Ok(lf.with_column(polars_expr.alias(name.as_str())))
        }
    }
}

/// Parse a simple arithmetic expression like "A + B - C" into a Polars Expr.
/// Supports +, -, *, / operators and column references or numeric literals.
fn parse_arithmetic_expr(expr_str: &str) -> Result<Expr, MongoCoreError> {
    let expr_str = expr_str.trim();
    // Tokenize: split on operators while keeping them
    let mut tokens: Vec<&str> = Vec::new();
    let mut last = 0;
    let bytes = expr_str.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'+' || b == b'-' || b == b'*' || b == b'/' {
            if i > last {
                tokens.push(expr_str[last..i].trim());
            }
            tokens.push(&expr_str[i..i + 1]);
            last = i + 1;
        }
    }
    if last < expr_str.len() {
        tokens.push(expr_str[last..].trim());
    }

    if tokens.is_empty() {
        return Err(MongoCoreError::IngestionError(
            "Empty arithmetic expression".to_string(),
        ));
    }

    fn token_to_expr(token: &str) -> Expr {
        if let Ok(n) = token.parse::<f64>() {
            lit(n)
        } else {
            col(token)
        }
    }

    let mut result = token_to_expr(tokens[0]);
    let mut i = 1;
    while i < tokens.len() - 1 {
        let op = tokens[i];
        let rhs = token_to_expr(tokens[i + 1]);
        result = match op {
            "+" => result + rhs,
            "-" => result - rhs,
            "*" => result * rhs,
            "/" => result / rhs,
            _ => {
                return Err(MongoCoreError::IngestionError(format!(
                    "Unknown operator in arithmetic expression: {op}"
                )))
            }
        };
        i += 2;
    }
    Ok(result)
}

/// Parse a simple filter expression like "age > 26" into a Polars Expr.
fn parse_filter_expr(expr_str: &str) -> Result<Expr, MongoCoreError> {
    // Check multi-char operators first
    let operators = [">=", "<=", "!=", "==", ">", "<"];
    let mut found_op = None;
    let mut split_pos = None;

    for op in &operators {
        if let Some(pos) = expr_str.find(op) {
            found_op = Some(*op);
            split_pos = Some(pos);
            break;
        }
    }

    let (op, pos) = match (found_op, split_pos) {
        (Some(op), Some(pos)) => (op, pos),
        _ => {
            return Err(MongoCoreError::IngestionError(format!(
                "No valid operator found in filter expression: {expr_str}"
            )));
        }
    };

    let column_name = expr_str[..pos].trim();
    let value_str = expr_str[pos + op.len()..].trim();

    let column_expr = col(column_name);
    let value_expr = parse_literal(value_str)?;

    let result = match op {
        ">" => column_expr.gt(value_expr),
        "<" => column_expr.lt(value_expr),
        ">=" => column_expr.gt_eq(value_expr),
        "<=" => column_expr.lt_eq(value_expr),
        "==" => column_expr.eq(value_expr),
        "!=" => column_expr.neq(value_expr),
        _ => unreachable!(),
    };

    Ok(result)
}

/// Parse a literal value into a Polars Expr.
fn parse_literal(s: &str) -> Result<Expr, MongoCoreError> {
    // Try i64
    if let Ok(v) = s.parse::<i64>() {
        return Ok(lit(v));
    }
    // Try f64
    if let Ok(v) = s.parse::<f64>() {
        return Ok(lit(v));
    }
    // Try boolean
    match s.to_lowercase().as_str() {
        "true" => return Ok(lit(true)),
        "false" => return Ok(lit(false)),
        _ => {}
    }
    // Quoted string
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        let inner = &s[1..s.len() - 1];
        return Ok(lit(inner.to_string()));
    }
    // Unquoted string
    Ok(lit(s.to_string()))
}

/// Parse a type name string into a Polars DataType.
fn parse_polars_dtype(s: &str) -> Result<DataType, MongoCoreError> {
    match s.to_lowercase().as_str() {
        "string" | "str" | "utf8" => Ok(DataType::String),
        "int32" | "i32" => Ok(DataType::Int32),
        "int64" | "i64" => Ok(DataType::Int64),
        "float32" | "f32" => Ok(DataType::Float32),
        "float64" | "f64" => Ok(DataType::Float64),
        "boolean" | "bool" => Ok(DataType::Boolean),
        "date" => Ok(DataType::Date),
        _ => Err(MongoCoreError::IngestionError(format!(
            "Unknown data type: {s}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_lf() -> LazyFrame {
        df! {
            "name" => ["Alice", "Bob", "Charlie"],
            "age" => [30i64, 25i64, 35i64],
            "score" => [95.5f64, 88.0f64, 72.0f64],
            "internal_id" => ["x1", "x2", "x3"],
        }
        .unwrap()
        .lazy()
    }

    #[test]
    fn test_rename() {
        let lf = sample_lf();
        let result = apply_expressions(lf, &["rename(name, full_name)".to_string()]).unwrap();
        let df = result.collect().unwrap();
        assert!(df.column("full_name").is_ok());
        assert!(df.column("name").is_err());
    }

    #[test]
    fn test_drop() {
        let lf = sample_lf();
        let result = apply_expressions(lf, &["drop(internal_id, score)".to_string()]).unwrap();
        let df = result.collect().unwrap();
        assert!(df.column("internal_id").is_err());
        assert!(df.column("score").is_err());
        assert!(df.column("name").is_ok());
    }

    #[test]
    fn test_filter() {
        let lf = sample_lf();
        let result = apply_expressions(lf, &["filter(age > 26)".to_string()]).unwrap();
        let df = result.collect().unwrap();
        assert_eq!(df.height(), 2);
    }

    #[test]
    fn test_cast() {
        let lf = sample_lf();
        let result = apply_expressions(lf, &["cast(age, Float64)".to_string()]).unwrap();
        let df = result.collect().unwrap();
        assert_eq!(df.column("age").unwrap().dtype(), &DataType::Float64);
    }

    #[test]
    fn test_select() {
        let lf = sample_lf();
        let result = apply_expressions(lf, &["select(name, age)".to_string()]).unwrap();
        let df = result.collect().unwrap();
        assert_eq!(df.width(), 2);
        assert!(df.column("name").is_ok());
        assert!(df.column("age").is_ok());
    }

    #[test]
    fn test_multiple_expressions() {
        let lf = sample_lf();
        let expressions = vec![
            "filter(age > 26)".to_string(),
            "drop(internal_id)".to_string(),
            "rename(name, full_name)".to_string(),
        ];
        let result = apply_expressions(lf, &expressions).unwrap();
        let df = result.collect().unwrap();
        assert_eq!(df.height(), 2);
        assert!(df.column("internal_id").is_err());
        assert!(df.column("full_name").is_ok());
    }

    #[test]
    fn test_invalid_expression() {
        let lf = sample_lf();
        let result = apply_expressions(lf, &["unknown_func(x)".to_string()]);
        assert!(result.is_err());
    }
}
