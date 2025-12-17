pub enum Arity {
    Binary,
    Unary,
    NoOp,
    NAry(usize),
}

pub trait Operator {
    fn arity(&self) -> Arity;
}
