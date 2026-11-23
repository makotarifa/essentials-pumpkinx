//! Economía propia persistida en `economy.json` (sustituye a Vault/PVault en Pumpkin).
//!
//! Los importes se guardan como enteros en céntavos para evitar errores de coma
//! flotante. `load_economy` solo lee: normaliza formatos antiguos en memoria y
//! marca `needs_migration`; el tick escribe la forma canónica más tarde.

use crate::store::{self};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, fs, io, path::Path};

fn schema() -> u32 {
    1
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PendingPay {
    pub to_uuid: String,
    pub to_name: String,
    pub amount: i64,
    pub at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Economy {
    #[serde(default = "schema")]
    pub schema: u32,
    /// UUID -> saldo en céntavos (nunca negativo).
    pub balances: BTreeMap<String, i64>,
    /// clave de item (minecraft:stone) -> precio por unidad en céntavos.
    pub worth: BTreeMap<String, i64>,
    /// UUID -> true si NO acepta pagos.
    pub pay_off: BTreeMap<String, bool>,
    /// UUID -> true si NO quiere confirmación al pagar.
    pub payconfirm_off: BTreeMap<String, bool>,
    /// UUID -> último pago pendiente de confirmación.
    pub pending_pay: BTreeMap<String, PendingPay>,
    #[serde(skip)]
    pub needs_migration: bool,
}

impl Default for Economy {
    fn default() -> Self {
        Self {
            schema: schema(),
            balances: BTreeMap::new(),
            worth: BTreeMap::new(),
            pay_off: BTreeMap::new(),
            payconfirm_off: BTreeMap::new(),
            pending_pay: BTreeMap::new(),
            needs_migration: false,
        }
    }
}

/// Parse a decimal amount into cents. Rejects negatives and non-finite values.
pub fn parse_money(raw: &str, decimals: u8) -> Result<i64, String> {
    let text = raw.trim();
    if text.is_empty() || text.starts_with('-') {
        return Err("La cantidad debe ser un número positivo.".into());
    }
    let value: f64 = text.parse().map_err(|_| format!("Cantidad inválida: '{raw}'."))?;
    if !value.is_finite() {
        return Err("Cantidad inválida.".into());
    }
    let factor = 10f64.powi(i32::from(decimals));
    let cents = (value * factor).round();
    if cents < 0.0 || cents > 9_000_000_000_000_000.0 {
        return Err("Cantidad fuera de rango.".into());
    }
    Ok(cents as i64)
}

pub fn format_money(cents: i64, symbol: &str, decimals: u8) -> String {
    let negative = cents < 0;
    let abs = cents.unsigned_abs();
    let factor = 10u64.pow(u32::from(decimals));
    let whole = abs / factor;
    let frac = abs % factor;
    let body = if decimals == 0 {
        format!("{whole}")
    } else {
        format!("{whole}.{frac:0width$}", width = usize::from(decimals))
    };
    if negative { format!("-{symbol}{body}") } else { format!("{symbol}{body}") }
}

/// Accept both `minecraft:stone` and bare `stone`.
pub fn normalize_item_key(raw: &str) -> String {
    let text = raw.trim().to_ascii_lowercase();
    if text.contains(':') { text } else { format!("minecraft:{text}") }
}

pub fn balance_of(economy: &Economy, uuid: &str) -> i64 {
    economy.balances.get(uuid).copied().unwrap_or(0).max(0)
}

pub fn set_balance(economy: &mut Economy, uuid: &str, amount: i64) -> Result<(), String> {
    if amount < 0 {
        return Err("El saldo no puede ser negativo.".into());
    }
    economy.balances.insert(uuid.to_string(), amount);
    Ok(())
}

pub fn adjust(economy: &mut Economy, uuid: &str, delta: i64) -> Result<(), String> {
    let next = balance_of(economy, uuid).saturating_add(delta);
    set_balance(economy, uuid, next)
}

/// Transfer between two UUIDs. Both balances must stay >= 0.
pub fn transfer(economy: &mut Economy, from: &str, to: &str, amount: i64) -> Result<(), String> {
    if amount <= 0 {
        return Err("La cantidad debe ser positiva.".into());
    }
    if balance_of(economy, from) < amount {
        return Err("Saldo insuficiente.".into());
    }
    adjust(economy, from, -amount)?;
    adjust(economy, to, amount)?;
    Ok(())
}

pub fn worth_of(economy: &Economy, key: &str) -> i64 {
    economy.worth.get(&normalize_item_key(key)).copied().unwrap_or(0).max(0)
}

fn unwrap_wrapper(value: &mut Value) -> bool {
    let Some(map) = value.as_object() else { return false };
    if map.len() != 1 { return false; }
    let Some(inner) = map.get("economy").cloned() else { return false; };
    *value = inner;
    true
}

/// Read-only load; migration is deferred to the periodic tick.
pub fn load_economy(path: &Path) -> Result<Economy, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) if text.trim().is_empty() => return Ok(Economy::default()),
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Economy::default()),
        Err(e) => return Err(format!("read {}: {e}", path.display())),
    };
    let mut value: Value = serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))?;
    let legacy = unwrap_wrapper(&mut value);
    let mut economy: Economy = serde_json::from_value(value).map_err(|e| format!("parse {}: {e}", path.display()))?;
    let old_schema = economy.schema < schema();
    if old_schema {
        economy.schema = schema();
    }
    economy.needs_migration = legacy || old_schema;
    Ok(economy)
}

pub fn save_economy(path: &Path, economy: &Economy) -> io::Result<()> {
    store::save_json(path, economy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("fusion-economy-test-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn money_roundtrip_and_validation() {
        assert_eq!(parse_money("1.5", 2), Ok(150));
        assert_eq!(parse_money("10", 2), Ok(1000));
        assert!(parse_money("-1", 2).is_err());
        assert!(parse_money("abc", 2).is_err());
        assert_eq!(format_money(150, "$", 2), "$1.50");
        assert_eq!(format_money(-150, "€", 2), "-€1.50");
    }

    #[test]
    fn item_keys() {
        assert_eq!(normalize_item_key(" STONE "), "minecraft:stone");
        assert_eq!(normalize_item_key("minecraft:DIAMOND"), "minecraft:diamond");
    }

    #[test]
    fn economy_never_negative() {
        let mut e = Economy::default();
        assert_eq!(balance_of(&e, "a"), 0);
        assert_eq!(adjust(&mut e, "a", 500), Ok(()));
        assert_eq!(adjust(&mut e, "a", -200), Ok(()));
        assert_eq!(balance_of(&e, "a"), 300);
        assert!(adjust(&mut e, "a", -400).is_err());
        assert_eq!(balance_of(&e, "a"), 300);
        assert!(set_balance(&mut e, "a", -1).is_err());
        assert_eq!(transfer(&mut e, "a", "b", 100), Ok(()));
        assert_eq!((balance_of(&e, "a"), balance_of(&e, "b")), (200, 100));
        assert!(transfer(&mut e, "a", "b", 500).is_err());
    }

    #[test]
    fn payment_confirmation_flow() {
        let mut e = Economy::default();
        set_balance(&mut e, "a", 1000).unwrap();
        e.pending_pay.insert("a".into(), PendingPay { to_uuid: "b".into(), to_name: "Bob".into(), amount: 250, at_ms: 1 });
        let pending = e.pending_pay.remove("a").unwrap();
        assert_eq!(transfer(&mut e, &"a".to_string(), &pending.to_uuid, pending.amount), Ok(()));
        assert_eq!(balance_of(&e, "a"), 750);
        assert_eq!(balance_of(&e, "b"), 250);
        assert!(e.pending_pay.get("a").is_none());
    }

    #[test]
    fn legacy_migration_is_deferred() {
        let d = tmpdir("legacy");
        let p = d.join("economy.json");
        let original = r#"{"economy":{"schema":0,"balances":{"a":123}}}"#;
        fs::write(&p, original).unwrap();
        let e = load_economy(&p).unwrap();
        assert!(e.needs_migration);
        assert_eq!(e.schema, schema());
        assert_eq!(balance_of(&e, "a"), 123);
        assert_eq!(fs::read_to_string(&p).unwrap(), original);
        save_economy(&p, &e).unwrap();
        let after = load_economy(&p).unwrap();
        assert!(!after.needs_migration);
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn corrupt_file_is_error_without_overwrite() {
        let d = tmpdir("corrupt");
        let p = d.join("economy.json");
        fs::write(&p, "{ nope").unwrap();
        assert!(load_economy(&p).is_err());
        assert_eq!(fs::read_to_string(&p).unwrap(), "{ nope");
        let _ = fs::remove_dir_all(d);
    }
}
