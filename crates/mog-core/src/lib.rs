//! Motor de lógica de mog 'em all.
//!
//! Todo lo que es cálculo, reglas o dominio vive aquí. Este crate no sabe nada
//! de la interfaz: devuelve datos y enums, nunca textos para mostrar. La capa
//! visual (`mog-ui`) es la que decide cómo se presenta cada valor y en qué idioma.

pub mod health;
pub mod ranks;
