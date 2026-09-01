// Safe, zero-dependency mathematical expression evaluator for LiteCast.

#[derive(Debug, Clone, PartialEq)]
pub struct CalculationResult {
    pub expression: String,
    pub result: f64,
    pub formatted_result: String,
}

pub fn evaluate_math_expression(input: &str) -> Option<CalculationResult> {
    let mut trimmed = input.trim();

    // Check if starts with "calc " or "="
    if let Some(rest) = trimmed.strip_prefix("calc ") {
        trimmed = rest.trim();
    } else if let Some(rest) = trimmed.strip_prefix("=") {
        trimmed = rest.trim();
    }

    if trimmed.is_empty() {
        return None;
    }

    // Handle "X% of Y" pattern
    if let Some(res) = parse_percent_of(trimmed) {
        return Some(res);
    }

    // Quick sanity check: Must contain at least one digit or math constant/function
    let has_math_char = trimmed
        .chars()
        .any(|c| c.is_ascii_digit() || "+-*/%^()".contains(c));
    if !has_math_char {
        return None;
    }

    // Attempt tokenization and evaluation
    let tokens = tokenize(trimmed)?;
    if tokens.is_empty() {
        return None;
    }

    // Avoid triggering on pure single numbers unless explicitly requested with calc/=
    let has_operator_or_func = tokens
        .iter()
        .any(|t| matches!(t, Token::Op(_) | Token::Func(_) | Token::Percent));

    let explicit_calc = input.trim().starts_with("calc ") || input.trim().starts_with('=');
    if !has_operator_or_func && !explicit_calc {
        return None;
    }

    let mut parser = Parser::new(tokens);
    let val = parser.parse_expression()?;

    if !parser.is_at_end() {
        return None;
    }

    if val.is_nan() || val.is_infinite() {
        return None;
    }

    let formatted = format_number(val);

    Some(CalculationResult {
        expression: trimmed.to_string(),
        result: val,
        formatted_result: formatted,
    })
}

fn parse_percent_of(input: &str) -> Option<CalculationResult> {
    let lower = input.to_lowercase();
    if let Some(idx) = lower.find("% of ") {
        let pct_str = &input[..idx].trim();
        let val_str = &input[idx + 5..].trim();

        let pct = evaluate_math_expression(pct_str)
            .map(|r| r.result)
            .or_else(|| pct_str.parse::<f64>().ok())?;
        let val = evaluate_math_expression(val_str)
            .map(|r| r.result)
            .or_else(|| val_str.parse::<f64>().ok())?;

        let res = (pct / 100.0) * val;
        return Some(CalculationResult {
            expression: input.to_string(),
            result: res,
            formatted_result: format_number(res),
        });
    }
    None
}

pub fn format_number(num: f64) -> String {
    if (num.fract() == 0.0) && num.abs() < 1e15 {
        format!("{}", num as i64)
    } else {
        let s = format!("{:.6}", num);
        let trimmed = s.trim_end_matches('0').trim_end_matches('.');
        trimmed.to_string()
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Num(f64),
    Op(char),
    Func(String),
    Percent,
    LParen,
    RParen,
    Comma,
}

fn tokenize(input: &str) -> Option<Vec<Token>> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }

        if c.is_ascii_digit() || (c == '.' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit())
        {
            let mut num_str = String::new();
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                num_str.push(chars[i]);
                i += 1;
            }
            let val = num_str.parse::<f64>().ok()?;
            tokens.push(Token::Num(val));
            continue;
        }

        if c.is_ascii_alphabetic() {
            let mut ident = String::new();
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                ident.push(chars[i].to_ascii_lowercase());
                i += 1;
            }
            match ident.as_str() {
                "pi" => tokens.push(Token::Num(std::f64::consts::PI)),
                "e" => tokens.push(Token::Num(std::f64::consts::E)),
                "sqrt" | "cbrt" | "sin" | "cos" | "tan" | "abs" | "round" | "floor" | "ceil"
                | "log" | "log10" | "log2" | "ln" | "rad" | "deg" => {
                    tokens.push(Token::Func(ident));
                }
                _ => return None, // Unknown identifier
            }
            continue;
        }

        match c {
            '+' | '-' | '*' | '/' | '^' => {
                tokens.push(Token::Op(c));
                i += 1;
            }
            'x' | 'X' => {
                // Support 'x' as multiplication if between numbers
                tokens.push(Token::Op('*'));
                i += 1;
            }
            '%' => {
                tokens.push(Token::Percent);
                i += 1;
            }
            '(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            ',' => {
                tokens.push(Token::Comma);
                i += 1;
            }
            _ => return None,
        }
    }

    Some(tokens)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&Token> {
        if !self.is_at_end() {
            self.pos += 1;
            self.tokens.get(self.pos - 1)
        } else {
            None
        }
    }

    fn parse_expression(&mut self) -> Option<f64> {
        self.parse_add_sub()
    }

    fn parse_add_sub(&mut self) -> Option<f64> {
        let mut left = self.parse_mul_div()?;

        while let Some(Token::Op(op)) = self.peek() {
            if *op == '+' || *op == '-' {
                let op = *op;
                self.advance();
                let right = self.parse_mul_div()?;
                if op == '+' {
                    left += right;
                } else {
                    left -= right;
                }
            } else {
                break;
            }
        }
        Some(left)
    }

    fn parse_mul_div(&mut self) -> Option<f64> {
        let mut left = self.parse_power()?;

        while let Some(tok) = self.peek() {
            match tok {
                Token::Op(op) if *op == '*' || *op == '/' => {
                    let op = *op;
                    self.advance();
                    let right = self.parse_power()?;
                    if op == '*' {
                        left *= right;
                    } else {
                        if right == 0.0 {
                            return None; // Division by zero
                        }
                        left /= right;
                    }
                }
                Token::Percent => {
                    self.advance();
                    left /= 100.0;
                }
                _ => break,
            }
        }
        Some(left)
    }

    fn parse_power(&mut self) -> Option<f64> {
        let left = self.parse_unary()?;

        if let Some(Token::Op('^')) = self.peek() {
            self.advance();
            let right = self.parse_power()?; // Right-associative
            Some(left.powf(right))
        } else {
            Some(left)
        }
    }

    fn parse_unary(&mut self) -> Option<f64> {
        if let Some(Token::Op(op)) = self.peek() {
            if *op == '+' {
                self.advance();
                return self.parse_unary();
            } else if *op == '-' {
                self.advance();
                let val = self.parse_unary()?;
                return Some(-val);
            }
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Option<f64> {
        let tok = self.advance()?.clone();
        match tok {
            Token::Num(n) => {
                // Check if followed by %
                if let Some(Token::Percent) = self.peek() {
                    self.advance();
                    Some(n / 100.0)
                } else {
                    Some(n)
                }
            }
            Token::Func(name) => {
                if let Some(Token::LParen) = self.peek() {
                    self.advance();
                    let arg = self.parse_expression()?;
                    if let Some(Token::RParen) = self.peek() {
                        self.advance();
                    } else {
                        return None;
                    }
                    match name.as_str() {
                        "sqrt" => {
                            if arg < 0.0 {
                                None
                            } else {
                                Some(arg.sqrt())
                            }
                        }
                        "cbrt" => Some(arg.cbrt()),
                        "sin" => Some(arg.sin()),
                        "cos" => Some(arg.cos()),
                        "tan" => Some(arg.tan()),
                        "abs" => Some(arg.abs()),
                        "round" => Some(arg.round()),
                        "floor" => Some(arg.floor()),
                        "ceil" => Some(arg.ceil()),
                        "ln" => {
                            if arg <= 0.0 {
                                None
                            } else {
                                Some(arg.ln())
                            }
                        }
                        "log" | "log10" => {
                            if arg <= 0.0 {
                                None
                            } else {
                                Some(arg.log10())
                            }
                        }
                        "log2" => {
                            if arg <= 0.0 {
                                None
                            } else {
                                Some(arg.log2())
                            }
                        }
                        "rad" => Some(arg.to_radians()),
                        "deg" => Some(arg.to_degrees()),
                        _ => None,
                    }
                } else {
                    None
                }
            }
            Token::LParen => {
                let val = self.parse_expression()?;
                if let Some(Token::RParen) = self.advance() {
                    Some(val)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_arithmetic() {
        let res = evaluate_math_expression("120 * 4 + 20").unwrap();
        assert_eq!(res.formatted_result, "500");

        let res2 = evaluate_math_expression("100 / 4 - 5").unwrap();
        assert_eq!(res2.formatted_result, "20");

        let res3 = evaluate_math_expression("2 ^ 8").unwrap();
        assert_eq!(res3.formatted_result, "256");

        let res4 = evaluate_math_expression("(10 + 20) * 3").unwrap();
        assert_eq!(res4.formatted_result, "90");
    }

    #[test]
    fn test_functions() {
        let res = evaluate_math_expression("sqrt(144)").unwrap();
        assert_eq!(res.formatted_result, "12");

        let res2 = evaluate_math_expression("abs(-42)").unwrap();
        assert_eq!(res2.formatted_result, "42");
    }

    #[test]
    fn test_percentages() {
        let res = evaluate_math_expression("20% of 150").unwrap();
        assert_eq!(res.formatted_result, "30");

        let res2 = evaluate_math_expression("50 * 10%").unwrap();
        assert_eq!(res2.formatted_result, "5");
    }

    #[test]
    fn test_invalid_and_non_math_returns_none() {
        assert!(evaluate_math_expression("notepad").is_none());
        assert!(evaluate_math_expression("hello world").is_none());
        assert!(evaluate_math_expression("10 / 0").is_none());
        assert!(evaluate_math_expression("").is_none());
    }
}
