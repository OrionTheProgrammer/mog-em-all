//! Motor de lógica de mog 'em all.
//!
//! Aquí van los cálculos, las reglas y el dominio. El crate no depende de la
//! interfaz: devuelve datos y enums, y `mog-ui` decide cómo mostrarlos y en qué
//! idioma.

pub mod catalog;
pub mod command;
pub mod health;
pub mod ranks;
pub mod schedule;
pub mod tasks;
pub mod training;
