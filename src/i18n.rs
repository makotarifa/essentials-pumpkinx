//! Centralised user-facing text. Internal logs stay in English.

use std::sync::atomic::{AtomicU8, Ordering};

static CURRENT: AtomicU8 = AtomicU8::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Es,
}

impl Lang {
    pub fn parse(value: &str) -> Self {
        if value.eq_ignore_ascii_case("es") || value.eq_ignore_ascii_case("es-es") || value.eq_ignore_ascii_case("spanish") {
            Self::Es
        } else {
            Self::En
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Es => "es",
        }
    }
}

pub fn set(lang: Lang) {
    CURRENT.store(if lang == Lang::Es { 1 } else { 0 }, Ordering::Relaxed);
}

pub fn set_from_str(value: &str) {
    set(Lang::parse(value));
}

pub fn current() -> Lang {
    if CURRENT.load(Ordering::Relaxed) == 1 { Lang::Es } else { Lang::En }
}

pub type Message = (&'static str, &'static str);

pub fn msg(message: Message) -> &'static str {
    if current() == Lang::Es { message.0 } else { message.1 }
}

pub fn msgf(message: Message, args: &[&str]) -> String {
    let mut out = msg(message).to_string();
    for arg in args {
        out = out.replacen("{}", arg, 1);
    }
    out
}

pub fn tr(es: &'static str, en: &'static str) -> &'static str {
    msg((es, en))
}

pub fn trf(es: &'static str, en: &'static str, args: &[&str]) -> String {
    msgf((es, en), args)
}

pub const ONLY_PLAYERS: Message = ("Solo jugadores.", "Players only.");
pub const UNAVAILABLE: Message = ("Estado no disponible.", "State unavailable.");
pub const PERMISSION_DENIED: Message = ("Sin permiso.", "Permission denied.");
pub const PLAYER_NOT_FOUND: Message = ("Jugador no conectado.", "Player is not online.");
pub const PLAYER_UNKNOWN: Message = ("Jugador desconocido.", "Unknown player.");
pub const WORLD_NOT_FOUND: Message = ("No existe el mundo.", "World does not exist.");
pub const INVALID_AMOUNT: Message = ("Cantidad no válida.", "Invalid amount.");
pub const CONFIRM_AGAIN: Message = ("Confirmación requerida: vuelve a ejecutar el comando para confirmar.", "Confirmation required: run the command again to confirm.");
pub const UNKNOWN_SUBCOMMAND: Message = ("Subcomando desconocido. Usa /essentials help.", "Unknown subcommand. Use /essentials help.");
pub const LANGUAGE_SET: Message = ("Idioma configurado en {}.", "Language set to {}.");
pub const LANGUAGE_USAGE: Message = ("Uso: /essentials language <en|es>.", "Usage: /essentials language <en|es>.");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_language() {
        assert_eq!(Lang::parse("en"), Lang::En);
        assert_eq!(Lang::parse("ES"), Lang::Es);
        assert_eq!(Lang::parse("spanish"), Lang::Es);
        assert_eq!(Lang::parse("de"), Lang::En);
    }

    #[test]
    fn translation_and_placeholders() {
        set(Lang::En);
        assert_eq!(msg(ONLY_PLAYERS), "Players only.");
        assert_eq!(msgf(LANGUAGE_SET, &["en"]), "Language set to en.");
        set(Lang::Es);
        assert_eq!(msg(ONLY_PLAYERS), "Solo jugadores.");
        assert_eq!(msgf(LANGUAGE_SET, &["es"]), "Idioma configurado en es.");
        set(Lang::En);
    }
}
