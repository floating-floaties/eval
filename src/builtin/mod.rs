
use crate::{Function, Functions, Value, to_value};
use crate::math::Math;
use crate::error::Error;

pub struct BuiltIn;

impl BuiltIn {
    pub fn create_builtins() -> Functions {
        let mut functions = Functions::new();
        functions.insert("min".to_owned(), create_min_function());
        functions.insert("max".to_owned(), create_max_function());
        functions.insert("len".to_owned(), create_len_function());
        functions.insert("is_empty".to_owned(), create_is_empty_function());
        functions.insert("array".to_owned(), create_array_function());
        functions
    }
}

#[derive(PartialEq)]
enum Compare {
    Min,
    Max,
}

fn create_min_function() -> Function {
    compare(Compare::Min)
}

fn create_max_function() -> Function {
    compare(Compare::Max)
}

fn compare(compare: Compare) -> Function {
    Function {
        max_args: None,
        min_args: Some(1),
        compiled: Box::new(move |values| {
            let mut prev: Result<Value, Error> = Err(Error::Custom("can't find min value."
                .to_owned()));

            for value in values {
                match value {
                    Value::Array(array) => {
                        for value in array {
                            if prev.is_ok() {
                                if compare == Compare::Min {
                                    if value.lt(prev.as_ref().unwrap())? == to_value(true) {
                                        prev = Ok(value)
                                    }
                                } else if value.gt(prev.as_ref().unwrap())? == to_value(true) {
                                    prev = Ok(value)
                                }
                            } else {
                                prev = Ok(value);
                            }
                        }
                    }
                    _ => {
                        if prev.is_ok() {
                            if compare == Compare::Min {
                                if value.lt(prev.as_ref().unwrap())? == to_value(true) {
                                    prev = Ok(value)
                                }
                            } else if value.gt(prev.as_ref().unwrap())? == to_value(true) {
                                prev = Ok(value)
                            }
                        } else {
                            prev = Ok(value);
                        }
                    }
                }
            }
            prev
        }),
    }
}


fn create_is_empty_function() -> Function {
    Function {
        max_args: Some(1),
        min_args: Some(1),
        compiled: Box::new(|values| match *values.first().unwrap() {
            Value::String(ref string) => Ok(to_value(string.is_empty())),
            Value::Array(ref array) => Ok(to_value(array.is_empty())),
            Value::Object(ref object) => Ok(to_value(object.is_empty())),
            Value::Null => Ok(to_value(true)),
            _ => Ok(to_value(false)),
        }),
    }
}

fn create_len_function() -> Function {
    Function {
        max_args: Some(1),
        min_args: Some(1),
        compiled: Box::new(|values| {
            let value = values.first().unwrap();
            match *value {
                Value::String(ref string) => Ok(to_value(string.len())),
                Value::Array(ref array) => Ok(to_value(array.len())),
                Value::Object(ref object) => Ok(to_value(object.len())),
                Value::Null => Ok(to_value(0)),
                _ => {
                    Err(Error::Custom(format!("len() only accept string, array, object and \
                                               null. But the given is: {:?}",
                                              value)))
                }
            }
        }),
    }
}

fn create_array_function() -> Function {
    Function::new(|values| Ok(to_value(values)))
}
