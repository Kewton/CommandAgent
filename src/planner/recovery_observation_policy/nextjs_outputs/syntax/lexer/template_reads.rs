//! A temp suffix may read the Node process id without changing process.cwd().
//! Keep the template opaque: this grants no authority to its dynamic value.
//! Every other interpolation still invalidates its referenced bindings.
use super::Token;

pub(super) fn is_process_id_read(tokens: &[Token]) -> bool {
    matches!(tokens, [Token::Word(object), Token::Symbol('.'), Token::Word(property)]
        if object == "process" && property == "pid")
}
