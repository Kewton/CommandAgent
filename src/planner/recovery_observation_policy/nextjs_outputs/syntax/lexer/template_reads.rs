//! A temp suffix may read the Node process id without changing process.cwd().
//! Keep the template opaque: this grants no authority to its dynamic value.
//! Every other interpolation still invalidates its referenced bindings.
use super::Token;

pub(super) fn is_process_id_read(tokens: &[Token]) -> bool {
    matches!(tokens, [Token::Word(object), Token::Symbol('.'), Token::Word(property)]
        if object == "process" && property == "pid")
}

// Defer receiver validation to Source: only an unshadowed Node path import can
// make this call a read. The template itself never becomes a path value.
pub(super) fn basename_receiver(tokens: &[Token]) -> Option<String> {
    match tokens {
        [
            Token::Word(receiver),
            Token::Symbol('.'),
            Token::Word(method),
            Token::Symbol('('),
            Token::Word(_),
            Token::Symbol(')'),
        ] if method == "basename" => Some(receiver.clone()),
        _ => None,
    }
}
