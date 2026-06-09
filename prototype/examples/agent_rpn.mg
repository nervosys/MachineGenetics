// Agent-built (dogfooding session): a small RPN (reverse-Polish) calculator.
// Exercises the general MechGen surface — sum types, records, pattern match,
// effect annotations, generics-free control flow — independent of the
// neural/Machine Language path.

// A token is either a number or one of four binary operators.
data Token = Num(f64) | Add | Sub | Mul | Div

// Evaluation can fail (stack underflow, divide-by-zero, leftover operands).
data EvalError = Underflow | DivZero | Leftover

// Apply one binary operator to the top two stack values.
pub fn apply(op: Token, a: f64, b: f64) -> R[f64, EvalError] {
    match op {
        Token.Add => Ok(a + b),
        Token.Sub => Ok(a - b),
        Token.Mul => Ok(a * b),
        Token.Div => if b == 0.0 { Err(EvalError.DivZero) } else { Ok(a / b) },
        Token.Num(_) => Err(EvalError.Leftover),
    }
}

// Evaluate a token stream, returning the single final value or an error.
pub fn eval(tokens: [Token]~) -> R[f64, EvalError] {
    var stack: [f64]~ = [];
    for tok in tokens {
        match tok {
            Token.Num(n) => stack.push(n),
            _ => {
                val b = match stack.pop() { Some(v) => v, None => return Err(EvalError.Underflow) };
                val a = match stack.pop() { Some(v) => v, None => return Err(EvalError.Underflow) };
                val r = apply(tok, a, b)?;
                stack.push(r);
            }
        }
    }
    match stack.len() {
        1 => Ok(stack[0]),
        _ => Err(EvalError.Leftover),
    }
}

// Demo: evaluate "3 4 + 2 *" = 14.
pub fn main() / io {
    val program: [Token]~ = [Token.Num(3.0), Token.Num(4.0), Token.Add, Token.Num(2.0), Token.Mul];
    match eval(program) {
        Ok(v) => println("result = {v}"),
        Err(_) => println("evaluation error"),
    }
}
