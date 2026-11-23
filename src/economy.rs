//! Self-contained economy persisted in `economy.json` (replaces Vault/PVault on Pumpkin).
//!
//! Amounts are stored as integer cents to avoid floating-point rounding
//! errors. `Economy` implements the shared [`FileStore`] pattern
//! (`crate::store::persist`): `load_economy` only reads, normalises legacy
//! formats in memory (marking `needs_migration`), and the tick writes the
//! canonical form later.

use crate::store::persist::FileStore;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, io, path::Path};

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
    /// UUID -> balance in cents (never negative).
    pub balances: BTreeMap<String, i64>,
    /// item key (minecraft:stone) -> price per unit in cents.
    pub worth: BTreeMap<String, i64>,
    /// UUID -> true if payments are DISABLED.
    pub pay_off: BTreeMap<String, bool>,
    /// UUID -> true if no confirmation is wanted when paying.
    pub payconfirm_off: BTreeMap<String, bool>,
    /// UUID -> last payment awaiting confirmation.
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

impl FileStore for Economy {
    const WRAPPER_KEY: &'static str = "economy";

    fn canonical_schema() -> u32 {
        schema()
    }
    fn stored_schema(&self) -> u32 {
        self.schema
    }
    fn bump_schema(&mut self) {
        self.schema = schema();
    }
    fn needs_migration(&self) -> bool {
        self.needs_migration
    }
    fn set_needs_migration(&mut self, needs: bool) {
        self.needs_migration = needs;
    }
}

/// Read-only load; migration is deferred to the periodic tick.
pub fn load_economy(path: &Path) -> Result<Economy, String> {
    Economy::load(path)
}

pub fn save_economy(path: &Path, economy: &Economy) -> io::Result<()> {
    Economy::save_atomic(path, economy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmpdir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pumpkinx-economy-test-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
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
        let mut economy = Economy::default();
        assert_eq!(balance_of(&economy, "a"), 0);
        assert_eq!(adjust(&mut economy, "a", 500), Ok(()));
        assert_eq!(adjust(&mut economy, "a", -200), Ok(()));
        assert_eq!(balance_of(&economy, "a"), 300);
        assert!(adjust(&mut economy, "a", -400).is_err());
        assert_eq!(balance_of(&economy, "a"), 300);
        assert!(set_balance(&mut economy, "a", -1).is_err());
        assert_eq!(transfer(&mut economy, "a", "b", 100), Ok(()));
        assert_eq!((balance_of(&economy, "a"), balance_of(&economy, "b")), (200, 100));
        assert!(transfer(&mut economy, "a", "b", 500).is_err());
    }

    #[test]
    fn payment_confirmation_flow() {
        let mut economy = Economy::default();
        set_balance(&mut economy, "a", 1000).unwrap();
        economy.pending_pay.insert("a".into(), PendingPay { to_uuid: "b".into(), to_name: "Bob".into(), amount: 250, at_ms: 1 });
        let pending = economy.pending_pay.remove("a").unwrap();
        assert_eq!(transfer(&mut economy, &"a".to_string(), &pending.to_uuid, pending.amount), Ok(()));
        assert_eq!(balance_of(&economy, "a"), 750);
        assert_eq!(balance_of(&economy, "b"), 250);
        assert!(economy.pending_pay.get("a").is_none());
    }

    #[test]
    fn legacy_migration_is_deferred() {
        let d = tmpdir("legacy");
        let p = d.join("economy.json");
        let original = r#"{"economy":{"schema":0,"balances":{"a":123}}}"#;
        fs::write(&p, original).unwrap();
        let economy = load_economy(&p).unwrap();
        assert!(economy.needs_migration);
        assert_eq!(economy.schema, schema());
        assert_eq!(balance_of(&economy, "a"), 123);
        assert_eq!(fs::read_to_string(&p).unwrap(), original);
        save_economy(&p, &economy).unwrap();
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
