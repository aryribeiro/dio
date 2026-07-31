use serde::Serialize;

use crate::format;

#[derive(Serialize, Clone)]
pub struct Asset {
    pub id: i64,
    pub name: String,
    pub unit_value: f64,
}

impl Asset {
    pub fn unit_value_brl(&self) -> String {
        format::brl(self.unit_value)
    }
}

pub struct UserRecord {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
}

/// Uma posição da carteira já enriquecida com os dados do ativo.
///
/// Vem do `JOIN` entre `holdings` e `assets`, então carrega tudo que a tela
/// precisa mostrar em uma linha da tabela.
#[derive(Serialize, Clone, Debug)]
pub struct Holding {
    pub asset_id: i64,
    pub name: String,
    pub unit_value: f64,
    pub quantity: f64,
}

impl Holding {
    /// Quanto essa posição vale hoje: quantidade × valor unitário.
    pub fn total_value(&self) -> f64 {
        self.quantity * self.unit_value
    }

    pub fn unit_value_brl(&self) -> String {
        format::brl(self.unit_value)
    }

    pub fn total_value_brl(&self) -> String {
        format::brl(self.total_value())
    }

    pub fn quantity_text(&self) -> String {
        format::quantity(self.quantity)
    }
}

/// A carteira inteira de uma pessoa, pronta para ser renderizada.
#[derive(Serialize)]
pub struct Wallet {
    pub holdings: Vec<Holding>,
}

impl Wallet {
    pub const fn new(holdings: Vec<Holding>) -> Self {
        Self { holdings }
    }

    /// Soma o valor de todas as posições.
    pub fn total_value(&self) -> f64 {
        self.holdings.iter().map(Holding::total_value).sum()
    }

    /// Quantos ativos diferentes compõem a carteira.
    pub fn len(&self) -> usize {
        self.holdings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.holdings.is_empty()
    }

    /// Fatia de cada posição no total da carteira, em porcentagem.
    ///
    /// Devolve `0.0` quando a carteira está zerada, evitando divisão por zero.
    pub fn share_of(&self, holding: &Holding) -> f64 {
        let total = self.total_value();
        if total == 0.0 {
            0.0
        } else {
            holding.total_value() / total * 100.0
        }
    }

    pub fn total_value_brl(&self) -> String {
        format::brl(self.total_value())
    }

    pub fn share_of_percent(&self, holding: &Holding) -> String {
        format::percent(self.share_of(holding))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn holding(name: &str, unit_value: f64, quantity: f64) -> Holding {
        Holding {
            asset_id: 1,
            name: name.to_string(),
            unit_value,
            quantity,
        }
    }

    #[test]
    fn total_value_multiplies_quantity_by_unit_value() {
        let bitcoin = holding("Bitcoin", 350_000.0, 0.5);
        assert_eq!(bitcoin.total_value(), 175_000.0);
    }

    #[test]
    fn wallet_total_sums_every_holding() {
        let wallet = Wallet::new(vec![
            holding("Bitcoin", 350_000.0, 0.5),
            holding("Ethereum", 12_400.0, 3.0),
        ]);

        assert_eq!(wallet.total_value(), 212_200.0);
        assert_eq!(wallet.len(), 2);
    }

    #[test]
    fn empty_wallet_is_worth_nothing() {
        let wallet = Wallet::new(vec![]);

        assert!(wallet.is_empty());
        assert_eq!(wallet.total_value(), 0.0);
    }

    #[test]
    fn share_of_empty_wallet_does_not_divide_by_zero() {
        let wallet = Wallet::new(vec![]);
        let orphan = holding("Bitcoin", 350_000.0, 0.5);

        assert_eq!(wallet.share_of(&orphan), 0.0);
    }

    #[test]
    fn shares_add_up_to_one_hundred_percent() {
        let wallet = Wallet::new(vec![
            holding("Bitcoin", 100.0, 3.0),
            holding("Ethereum", 100.0, 1.0),
        ]);

        assert_eq!(wallet.share_of(&wallet.holdings[0]), 75.0);
        assert_eq!(wallet.share_of(&wallet.holdings[1]), 25.0);
    }
}
