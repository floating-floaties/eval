
use crate::operator::Operator;
use crate::error::Error;
use crate::Function;


#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub operator: Operator,
    pub children: Vec<Node>,
    pub closed: bool,
}

impl Node {
    pub fn new(operator: Operator) -> Node {
        Node {
            operator,
            children: Vec::new(),
            closed: false,
        }
    }

    pub fn check_function_args(&self, function: &Function) -> Result<(), Error> {
        let args_length = self.children.len();

        if let Some(len) = function.max_args {
            if args_length > len {
                return Err(Error::ArgumentsGreater(len));
            }
        }

        if let Some(len) = function.min_args {
            if args_length < len {
                return Err(Error::ArgumentsLess(len));
            }
        }

        Ok(())
    }

    pub fn is_enough(&self) -> bool {
        let num = self.operator.get_max_args();
        if let Some(value) = num {
            self.children.len() == value
        } else {
            false
        }
    }

    pub fn is_value_or_full_children(&self) -> bool {
        if self.operator.is_value_or_ident() {
            true
        } else if self.operator.can_have_child() {
            if self.closed { true } else { self.is_enough() }
        } else {
            false
        }
    }

    pub fn is_unclosed_arithmetic(&self) -> bool {
        !self.closed && self.operator.can_have_child() && self.operator.can_have_child()
    }

    pub fn is_unclosed_function(&self) -> bool {
        match self.operator {
            Operator::Function(_) => !self.closed,
            _ => false,
        }
    }

    pub fn is_unclosed_square_bracket(&self) -> bool {
        match self.operator {
            Operator::LeftSquareBracket(_) => !self.closed,
            _ => false,
        }
    }

    pub fn is_left_square_bracket(&self) -> bool {
        matches!(self.operator, Operator::LeftSquareBracket(_))
    }

    pub fn is_dot(&self) -> bool {
        matches!(self.operator, Operator::Dot(_))
    }

    pub fn add_child(&mut self, node: Node) {
        self.children.push(node);
    }

    pub fn get_first_child(&self) -> Node {
        self.children.first().unwrap().clone()
    }

    pub fn get_last_child(&self) -> Node {
        self.children.last().unwrap().clone()
    }

    pub fn move_out_last_node(&mut self) -> Node {
        self.children.pop().unwrap()
    }
}
