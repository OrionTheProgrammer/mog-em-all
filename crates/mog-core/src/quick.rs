//! Entrada rápida única: decide si lo escrito es un comando, un movimiento de dinero o una tarea.

use crate::command::{self, Command};
use crate::finance::{self, Currency};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickKind {
    Empty,
    Command,
    Money,
    Task,
}

/// - Comando: `ir finanzas`, `imc 78 176`, `ayuda`, o una palabra suelta como `ir`.
/// - Dinero: empieza con un monto (`12.500 almuerzo`, `+20.000 venta`, `$4990 café`).
/// - Tarea: todo lo demás (`pagar luz mañana`).
pub fn classify(text: &str, currency: Currency) -> QuickKind {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return QuickKind::Empty;
    }
    let single_word = !trimmed.contains(char::is_whitespace);
    match command::parse(trimmed) {
        Command::Go(_) | Command::Bmi { .. } | Command::Language(_) | Command::Help => {
            return QuickKind::Command;
        }
        Command::Usage(_) if single_word => return QuickKind::Command,
        _ => {}
    }
    let first = trimmed
        .trim_start_matches(['+', '-', '$'])
        .chars()
        .next()
        .unwrap_or(' ');
    if first.is_ascii_digit() && finance::parse_quick(trimmed, currency, 0).is_some() {
        return QuickKind::Money;
    }
    QuickKind::Task
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(text: &str) -> QuickKind {
        classify(text, finance::currency("CLP"))
    }

    #[test]
    fn commands() {
        assert_eq!(kind("ir finanzas"), QuickKind::Command);
        assert_eq!(kind("imc 78 176"), QuickKind::Command);
        assert_eq!(kind("ayuda"), QuickKind::Command);
        assert_eq!(kind("ir"), QuickKind::Command);
    }

    #[test]
    fn money() {
        assert_eq!(kind("12.500 almuerzo"), QuickKind::Money);
        assert_eq!(kind("+20.000 venta polera"), QuickKind::Money);
        assert_eq!(kind("$4990 cafe"), QuickKind::Money);
        assert_eq!(kind("15k supermercado"), QuickKind::Money);
    }

    #[test]
    fn tasks() {
        assert_eq!(kind("pagar luz mañana"), QuickKind::Task);
        // "ir" con más palabras que no son un módulo es una tarea.
        assert_eq!(kind("ir al banco el viernes"), QuickKind::Task);
        assert_eq!(kind("comprar 2 litros de leche"), QuickKind::Task);
        assert_eq!(kind("  "), QuickKind::Empty);
    }
}
