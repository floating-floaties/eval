use crate::builtin::BuiltIn;
use crate::error::Error;
use crate::math::Math;
use crate::node::Node;
use crate::operator::Operator;
use crate::Compiled;
use crate::{to_value, ConstFunctions};
use crate::{Context, Functions};
use serde_json::Value;
use std::cell::RefCell;
use std::clone::Clone;
use std::rc::Rc;
use std::str::FromStr;

#[derive(Default)]
pub struct Tree {
    pub raw: String,
    pub pos: Vec<usize>,
    pub operators: Vec<Operator>,
    pub node: Option<Node>,
}

impl Tree {
    pub fn new<T: Into<String>>(raw: T) -> Tree {
        Tree {
            raw: raw.into(),
            ..Default::default()
        }
    }

    pub fn parse_pos(&mut self) -> Result<(), Error> {
        let mut found_quote = false;
        let mut pos = Vec::new();

        for (index, cur) in self.raw.chars().enumerate() {
            match cur {
                '(' | ')' | '+' | '-' | '*' | '/' | ',' | ' ' | '!' | '=' | '>' | '<' | '\''
                | '[' | ']' | '.' | '%' | '&' | '|' => {
                    if !found_quote {
                        pos.push(index);
                        pos.push(index + 1);
                    }
                }
                '"' => {
                    found_quote = !found_quote;
                    pos.push(index);
                    pos.push(index + 1);
                }
                _ => (),
            }
        }

        pos.push(self.raw.len());

        self.pos = pos;
        Ok(())
    }

    pub fn parse_operators(&mut self) -> Result<(), Error> {
        let mut operators = Vec::new();
        let mut start;
        let mut end = 0;
        let mut parenthesis = 0;
        let mut quote = None;
        let mut prev = String::new();
        let mut number = String::new();

        for pos_ref in &self.pos {
            let pos = *pos_ref;
            if pos == 0 {
                continue;
            } else {
                start = end;
                end = pos;
            }

            let raw = self.raw[start..end].to_owned();

            if raw.is_empty() {
                continue;
            }

            let operator = Operator::from_str(&raw).unwrap();
            match operator {
                Operator::DoubleQuotes | Operator::SingleQuote => {
                    if quote.is_some() {
                        if quote.as_ref() == Some(&operator) {
                            operators.push(Operator::Value(to_value(&prev)));
                            prev.clear();
                            quote = None;
                            continue;
                        }
                    } else {
                        quote = Some(operator);
                        prev.clear();
                        continue;
                    }
                }
                _ => (),
            };

            if quote.is_some() {
                prev += &raw;
                continue;
            }

            if parse_number(&raw).is_some() || operator.is_dot() {
                number += &raw;
                continue;
            } else if !number.is_empty() {
                operators.push(Operator::from_str(&number).unwrap());
                number.clear();
            }

            if raw == "=" {
                if prev == "!" || prev == ">" || prev == "<" || prev == "=" {
                    prev.push('=');
                    operators.push(Operator::from_str(&prev).unwrap());
                    prev.clear();
                } else {
                    prev = raw;
                }
                continue;
            } else if raw == "!" || raw == ">" || raw == "<" {
                if prev == "!" || prev == ">" || prev == "<" {
                    operators.push(Operator::from_str(&prev).unwrap());
                    prev.clear();
                } else {
                    prev = raw;
                }
                continue;
            } else if prev == "!" || prev == ">" || prev == "<" {
                operators.push(Operator::from_str(&prev).unwrap());
                prev.clear();
            }

            if (raw == "&" || raw == "|") && (prev == "&" || prev == "|") {
                if raw == prev {
                    prev.push_str(&raw);
                    operators.push(Operator::from_str(&prev).unwrap());
                    prev.clear();
                    continue;
                } else {
                    return Err(Error::UnsupportedOperator(prev));
                }
            } else if raw == "&" || raw == "|" {
                prev = raw;
                continue;
            }

            match operator {
                Operator::LeftParenthesis => {
                    parenthesis += 1;

                    if !operators.is_empty() {
                        let prev_operator = operators.pop().unwrap();
                        if prev_operator.is_identifier() {
                            operators.push(Operator::Function(
                                prev_operator.get_identifier().to_owned(),
                            ));
                            operators.push(operator);
                            continue;
                        } else {
                            operators.push(prev_operator);
                        }
                    }
                }
                Operator::RightParenthesis => parenthesis -= 1,
                Operator::WhiteSpace => continue,
                _ => (),
            }

            prev = raw;
            operators.push(operator);
        }

        if !number.is_empty() {
            operators.push(Operator::from_str(&number).unwrap());
        }

        if parenthesis != 0 {
            Err(Error::UnpairedBrackets)
        } else {
            self.operators = operators;
            Ok(())
        }
    }

    pub fn parse_node(&mut self) -> Result<(), Error> {
        let mut parsing_nodes = Vec::<Node>::new();

        for operator in &self.operators {
            match *operator {
                Operator::Add(priority)
                | Operator::Sub(priority)
                | Operator::Mul(priority)
                | Operator::Div(priority)
                | Operator::Not(priority)
                | Operator::Eq(priority)
                | Operator::Ne(priority)
                | Operator::Gt(priority)
                | Operator::Lt(priority)
                | Operator::Ge(priority)
                | Operator::And(priority)
                | Operator::Or(priority)
                | Operator::Le(priority)
                | Operator::Dot(priority)
                | Operator::LeftSquareBracket(priority)
                | Operator::Rem(priority) => {
                    if !parsing_nodes.is_empty() {
                        let prev = parsing_nodes.pop().unwrap();
                        if prev.is_value_or_full_children() {
                            if prev.operator.get_priority() < priority && !prev.closed {
                                parsing_nodes.extend_from_slice(&rob_to(prev, operator.to_node()));
                            } else {
                                parsing_nodes.push(operator.children_to_node(vec![prev]));
                            }
                        } else if prev.operator.can_at_beginning() {
                            parsing_nodes.push(prev);
                            parsing_nodes.push(operator.to_node());
                        } else {
                            return Err(Error::DuplicateOperatorNode);
                        }
                    } else if operator.can_at_beginning() {
                        parsing_nodes.push(operator.to_node());
                    } else {
                        return Err(Error::StartWithNonValueOperator);
                    }
                }
                Operator::Function(_) | Operator::LeftParenthesis => {
                    parsing_nodes.push(operator.to_node())
                }
                Operator::Comma => close_comma(&mut parsing_nodes)?,
                Operator::RightParenthesis | Operator::RightSquareBracket => {
                    close_bracket(&mut parsing_nodes, operator.get_left())?
                }
                Operator::Value(_) | Operator::Identifier(_) => {
                    append_value_to_last_node(&mut parsing_nodes, operator)?
                }
                _ => (),
            }
        }

        self.node = Some(get_final_node(parsing_nodes)?);
        Ok(())
    }

    pub fn compile(mut self) -> Result<Compiled, Error> {
        self.parse_pos()?;
        self.parse_operators()?;
        self.parse_node()?;
        let node = self.node.unwrap();
        let builtin = BuiltIn::create_builtins();

        Ok(Box::new(
            move |contexts, functions, const_functions| -> Result<Value, Error> {
                return exec_node(&node, &builtin, contexts, functions, const_functions);

            #[rustfmt::skip]
            fn exec_node(node: &Node,
                         builtin: &Functions,
                         contexts: &[Context],
                         functions: &Functions,
                         const_functions: Rc<RefCell<ConstFunctions>>,)
                         -> Result<Value, Error> {
                match node.operator {
                    Operator::Add(_) => {
                        exec_node(&node.get_first_child(), builtin, contexts, functions, Rc::clone(&const_functions))
                            ?
                            .add(&exec_node(&node.get_last_child(), builtin, contexts, functions, Rc::clone(&const_functions))?)
                    }
                    Operator::Mul(_) => {
                        exec_node(&node.get_first_child(), builtin, contexts, functions, Rc::clone(&const_functions))
                            ?
                            .mul(&exec_node(&node.get_last_child(), builtin, contexts, functions, Rc::clone(&const_functions))?)
                    }
                    Operator::Sub(_) => {
                        exec_node(&node.get_first_child(), builtin, contexts, functions, Rc::clone(&const_functions))
                            ?
                            .sub(&exec_node(&node.get_last_child(), builtin, contexts, functions, Rc::clone(&const_functions))?)
                    }
                    Operator::Div(_) => {
                        exec_node(&node.get_first_child(), builtin, contexts, functions, Rc::clone(&const_functions))
                            ?
                            .div(&exec_node(&node.get_last_child(), builtin, contexts, functions, Rc::clone(&const_functions))?)
                    }
                    Operator::Rem(_) => {
                        exec_node(&node.get_first_child(), builtin, contexts, functions, Rc::clone(&const_functions))
                            ?
                            .rem(&exec_node(&node.get_last_child(), builtin, contexts, functions, Rc::clone(&const_functions))?)
                    }
                    Operator::Eq(_) => {
                        Math::eq(&exec_node(&node.get_first_child(), builtin, contexts, functions, Rc::clone(&const_functions))?,
                                 &exec_node(&node.get_last_child(), builtin, contexts, functions, Rc::clone(&const_functions))?)
                    }
                    Operator::Ne(_) => {
                        Math::ne(&exec_node(&node.get_first_child(), builtin, contexts, functions, Rc::clone(&const_functions))?,
                                 &exec_node(&node.get_last_child(), builtin, contexts, functions, Rc::clone(&const_functions))?)
                    }
                    Operator::Gt(_) => {
                        exec_node(&node.get_first_child(), builtin, contexts, functions, Rc::clone(&const_functions))
                            ?
                            .gt(&exec_node(&node.get_last_child(), builtin, contexts, functions, Rc::clone(&const_functions))?)
                    }
                    Operator::Lt(_) => {
                        exec_node(&node.get_first_child(), builtin, contexts, functions, Rc::clone(&const_functions))
                            ?
                            .lt(&exec_node(&node.get_last_child(), builtin, contexts, functions, Rc::clone(&const_functions))?)
                    }
                    Operator::Ge(_) => {
                        exec_node(&node.get_first_child(), builtin, contexts, functions, Rc::clone(&const_functions))
                            ?
                            .ge(&exec_node(&node.get_last_child(), builtin, contexts, functions, Rc::clone(&const_functions))?)
                    }
                    Operator::Le(_) => {
                        exec_node(&node.get_first_child(), builtin, contexts, functions, Rc::clone(&const_functions))
                            ?
                            .le(&exec_node(&node.get_last_child(), builtin, contexts, functions, Rc::clone(&const_functions))?)
                    }
                    Operator::And(_) => {
                        exec_node(&node.get_first_child(), builtin, contexts, functions, Rc::clone(&const_functions))
                            ?
                            .and(&exec_node(&node.get_last_child(), builtin, contexts, functions, Rc::clone(&const_functions))?)
                    }
                    Operator::Or(_) => {
                        exec_node(&node.get_first_child(), builtin, contexts, functions, Rc::clone(&const_functions))
                            ?
                            .or(&exec_node(&node.get_last_child(), builtin, contexts, functions, Rc::clone(&const_functions))?)
                    }
                    Operator::Function(ref ident) => {
                        let function_option = if functions.contains_key(ident) {
                            functions.get(ident)
                        } else {
                            builtin.get(ident)
                        };
                        let mut values = Vec::new();
                        for node in &node.children {
                            values.push(exec_node(node, builtin, contexts, functions, Rc::clone(&const_functions))?);
                        }

                        if let Some(fo) = function_option {
                            let function = fo;
                            node.check_function_args(function)?;
                            (function.compiled)(values)
                        } else if let Some(f) = const_functions.borrow().get(ident){
                            (f.compiled)(values)
                        } else {
                            Err(Error::FunctionNotExists(ident.to_owned()))
                        }
                    }
                    Operator::Value(ref value) => Ok(value.clone()),
                    Operator::Not(_) => {
                        let value =
                            exec_node(&node.get_first_child(), builtin, contexts, functions, Rc::clone(&const_functions))?;
                        match value {
                            Value::Bool(boolean) => Ok(Value::Bool(!boolean)),
                            Value::Null => Ok(Value::Bool(true)),
                            _ => Err(Error::ExpectedBoolean(value)),
                        }
                    }
                    Operator::Dot(_) => {
                        let mut value = None;
                        for child in &node.children {
                            if value.is_none() {
                                let name = exec_node(child, builtin, contexts, functions, Rc::clone(&const_functions))?;
                                if name.is_string() {
                                    value = find(contexts, name.as_str().unwrap());
                                    if value.is_none() {
                                        return Ok(Value::Null);
                                    }
                                } else if name.is_object() {
                                    value = Some(name);
                                } else if name.is_null() {
                                    return Ok(Value::Null);
                                } else {
                                    return Err(Error::ExpectedObject);
                                }
                            } else if child.operator.is_identifier() {
                                value = value.as_ref()
                                    .unwrap()
                                    .get(child.operator.get_identifier())
                                    .cloned();
                            } else {
                                return Err(Error::ExpectedIdentifier);
                            }
                        }

                        if let Some(v) = value {
                            Ok(v)
                        } else {
                            Ok(Value::Null)
                        }
                    }
                    Operator::LeftSquareBracket(_) => {
                        let mut value = None;
                        for child in &node.children {
                            let name = exec_node(child, builtin, contexts, functions, Rc::clone(&const_functions))?;
                            if value.is_none() {
                                if name.is_string() {
                                    value = find(contexts, name.as_str().unwrap());
                                    if value.is_none() {
                                        return Ok(Value::Null);
                                    }
                                } else if name.is_array() || name.is_object(){
                                    value = Some(name);
                                } else if name.is_null() {
                                    return Ok(Value::Null);
                                } else {
                                    return Err(Error::ExpectedArray);
                                }
                            } else if value.as_ref().unwrap().is_object() {
                                if name.is_string() {
                                    value = value.as_ref()
                                        .unwrap()
                                        .get(name.as_str().unwrap())
                                        .cloned();
                                } else {
                                    return Err(Error::ExpectedIdentifier);
                                }
                            } else if name.is_u64() {
                                if value.as_ref().unwrap().is_array() {
                                    value = value.as_ref()
                                        .unwrap()
                                        .as_array()
                                        .unwrap()
                                        .get(name.as_u64().unwrap() as usize)
                                        .cloned();
                                } else {
                                    return Err(Error::ExpectedArray);
                                }
                            } else {
                                return Err(Error::ExpectedNumber);
                            }
                        }
                        if let Some(v) = value {
                            Ok(v)
                        } else {
                            Ok(Value::Null)
                        }
                    }
                    Operator::Identifier(ref ident) => {
                        let number = parse_number(ident);
                        if let Some(n) = number {
                            Ok(n)
                        } else if is_range(ident) {
                            parse_range(ident)
                        } else {
                            match find(contexts, ident) {
                                Some(value) => Ok(value),
                                None => Ok(Value::Null),
                            }
                        }
                    }
                    _ => Err(Error::CanNotExec(node.operator.clone())),
                }
            }
            },
        ))
    }
}

fn append_value_to_last_node(
    parsing_nodes: &mut Vec<Node>,
    operator: &Operator,
) -> Result<(), Error> {
    let mut node = operator.to_node();
    node.closed = true;

    if let Some(mut prev) = parsing_nodes.pop() {
        if prev.is_dot() {
            prev.add_child(node);
            prev.closed = true;
            parsing_nodes.push(prev);
        } else if prev.is_left_square_bracket() {
            parsing_nodes.push(prev);
            parsing_nodes.push(node);
        } else if prev.is_value_or_full_children() {
            return Err(Error::DuplicateValueNode);
        } else if prev.is_enough() {
            parsing_nodes.push(prev);
            parsing_nodes.push(node);
        } else if prev.operator.can_have_child() {
            prev.add_child(node);
            parsing_nodes.push(prev);
        } else {
            return Err(Error::CanNotAddChild);
        }
    } else {
        parsing_nodes.push(node);
    }

    Ok(())
}

fn get_final_node(mut parsing_nodes: Vec<Node>) -> Result<Node, Error> {
    if parsing_nodes.is_empty() {
        return Err(Error::NoFinalNode);
    }

    while parsing_nodes.len() != 1 {
        let last = parsing_nodes.pop().unwrap();
        let mut prev = parsing_nodes.pop().unwrap();
        if prev.operator.can_have_child() {
            prev.add_child(last);
            parsing_nodes.push(prev);
        } else {
            return Err(Error::CanNotAddChild);
        }
    }

    Ok(parsing_nodes.pop().unwrap())
}

fn close_bracket(parsing_nodes: &mut Vec<Node>, bracket: Operator) -> Result<(), Error> {
    loop {
        let mut current = parsing_nodes.pop().unwrap();
        let mut prev = parsing_nodes.pop().unwrap();

        if current.operator.is_left_square_bracket() {
            return Err(Error::BracketNotWithFunction);
        } else if prev.operator.is_left_square_bracket() {
            prev.add_child(current);
            prev.closed = true;
            parsing_nodes.push(prev);
            break;
        } else if current.operator == bracket {
            if prev.is_unclosed_function() {
                prev.closed = true;
                parsing_nodes.push(prev);
                break;
            } else {
                return Err(Error::BracketNotWithFunction);
            }
        } else if prev.operator == bracket {
            if !current.closed {
                current.closed = true;
            }

            if let Some(mut p) = parsing_nodes.pop() {
                if p.is_unclosed_function() {
                    p.closed = true;
                    p.add_child(current);
                    parsing_nodes.push(p);
                } else if p.is_unclosed_arithmetic() {
                    p.add_child(current);
                    parsing_nodes.push(p);
                } else {
                    parsing_nodes.push(p);
                    parsing_nodes.push(current);
                }
            } else {
                parsing_nodes.push(current);
            }
            break;
        } else if !prev.closed {
            prev.add_child(current);
            if prev.is_enough() {
                prev.closed = true;
            }

            if !parsing_nodes.is_empty() {
                parsing_nodes.push(prev);
            } else {
                return Err(Error::StartWithNonValueOperator);
            }
        } else {
            return Err(Error::StartWithNonValueOperator);
        }
    }

    Ok(())
}

fn close_comma(parsing_nodes: &mut Vec<Node>) -> Result<(), Error> {
    if parsing_nodes.len() < 2 {
        return Err(Error::CommaNotWithFunction);
    }

    loop {
        let current = parsing_nodes.pop().unwrap();
        let mut prev = parsing_nodes.pop().unwrap();

        if current.operator == Operator::Comma {
            parsing_nodes.push(prev);
            break;
        } else if current.operator.is_left() {
            parsing_nodes.push(prev);
            parsing_nodes.push(current);
            break;
        } else if prev.operator.is_left() {
            if let Some(mut p) = parsing_nodes.pop() {
                if p.is_unclosed_function() {
                    p.add_child(current);
                    parsing_nodes.push(p);
                    parsing_nodes.push(prev);
                    break;
                } else {
                    return Err(Error::CommaNotWithFunction);
                }
            } else {
                return Err(Error::CommaNotWithFunction);
            }
        } else if !prev.closed {
            prev.add_child(current);
            if prev.is_enough() {
                prev.closed = true;
            }

            if !parsing_nodes.is_empty() {
                parsing_nodes.push(prev);
            } else {
                return Err(Error::StartWithNonValueOperator);
            }
        } else {
            return Err(Error::StartWithNonValueOperator);
        }
    }
    Ok(())
}

fn rob_to(mut was_robed: Node, mut robber: Node) -> Vec<Node> {
    let move_out_node = was_robed.move_out_last_node();
    robber.add_child(move_out_node);
    vec![was_robed, robber]
}

fn find(contexts: &[Context], key: &str) -> Option<Value> {
    for context in contexts.iter().rev() {
        match context.get(key) {
            Some(value) => return Some(value.clone()),
            None => continue,
        }
    }

    None
}

fn is_range(ident: &str) -> bool {
    ident.contains("..")
}

fn parse_range(ident: &str) -> Result<Value, Error> {
    let segments = ident.split("..").collect::<Vec<_>>();
    if segments.len() != 2 {
        Err(Error::InvalidRange(ident.to_owned()))
    } else {
        let start = segments[0].parse::<i64>();
        let end = segments[1].parse::<i64>();

        match (start, end) {
            (Ok(start), Ok(end)) => {
                let mut array = Vec::new();
                for n in start..end {
                    array.push(n);
                }
                Ok(to_value(array))
            }
            _ => {
                Err(Error::InvalidRange(ident.to_owned()))
            }
        }
    }
}

fn parse_number(ident: &str) -> Option<Value> {
    let number = ident.parse::<u64>();
    if let Ok(n) = number {
        return Some(to_value(n));
    }

    let number = ident.parse::<i64>();
    if let Ok(n) = number {
        return Some(to_value(n));
    }

    let number = ident.parse::<f64>();
    if let Ok(n) = number {
        return Some(to_value(n));
    }

    None
}
