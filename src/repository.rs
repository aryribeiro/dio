use std::convert::Infallible;

use axum::extract::FromRequestParts;
use sqlx::PgPool;

use crate::{
    app::AppState,
    error::AppError,
    models::{Asset, Holding, UserRecord, Wallet},
};

pub struct Repository {
    db: PgPool,
}

impl Repository {
    pub async fn list_assets(&self) -> sqlx::Result<Vec<Asset>> {
        sqlx::query_as!(
            Asset,
            "SELECT id, name, unit_value
             FROM assets
             ORDER BY name;"
        )
        .fetch_all(&self.db)
        .await
    }

    pub async fn create_asset(&self, name: String, unit_value: f64) -> sqlx::Result<Asset> {
        sqlx::query_as!(
            Asset,
            "INSERT INTO assets (name, unit_value)
             VALUES ($1, $2)
             RETURNING id, name, unit_value;",
            name,
            unit_value
        )
        .fetch_one(&self.db)
        .await
    }

    pub async fn update_asset(
        &self,
        asset_id: i64,
        name: Option<String>,
        unit_value: Option<f64>,
    ) -> sqlx::Result<Option<Asset>> {
        sqlx::query_as!(
            Asset,
            "UPDATE assets
             SET name=COALESCE($2, name),
                 unit_value=COALESCE($3, unit_value)
             WHERE id=$1
             RETURNING id, name, unit_value;",
            asset_id,
            name,
            unit_value
        )
        .fetch_optional(&self.db)
        .await
    }

    pub async fn add_user(&self, username: &str, password_hash: &str) -> sqlx::Result<UserRecord> {
        sqlx::query_as!(
            UserRecord,
            "INSERT INTO users (username, password_hash)
             VALUES ($1, $2)
             RETURNING id, username, password_hash;",
            username,
            password_hash,
        )
        .fetch_one(&self.db)
        .await
    }

    pub async fn get_user_by_name(&self, username: &str) -> sqlx::Result<Option<UserRecord>> {
        sqlx::query_as!(
            UserRecord,
            "SELECT id, username, password_hash
             FROM users
             WHERE username = $1;",
            username
        )
        .fetch_optional(&self.db)
        .await
    }

    /// Monta a carteira de uma pessoa: cada posição já vem com o nome e o valor
    /// unitário atuais do ativo, que é tudo que o dashboard precisa.
    pub async fn get_wallet(&self, user_id: i64) -> sqlx::Result<Wallet> {
        let holdings = sqlx::query_as!(
            Holding,
            r#"SELECT h.asset_id AS "asset_id!",
                      a.name AS "name!",
                      a.unit_value AS "unit_value!",
                      h.quantity AS "quantity!"
               FROM holdings h
               JOIN assets a ON a.id = h.asset_id
               WHERE h.user_id = $1
               ORDER BY a.name;"#,
            user_id
        )
        .fetch_all(&self.db)
        .await?;

        Ok(Wallet::new(holdings))
    }

    /// Adiciona `quantity` do ativo à carteira, somando ao que já existe.
    ///
    /// O `ON CONFLICT` deixa a operação atômica: duas compras simultâneas do
    /// mesmo ativo se somam em vez de uma sobrescrever a outra.
    pub async fn buy_asset(
        &self,
        user_id: i64,
        asset_id: i64,
        quantity: f64,
    ) -> Result<Holding, AppError> {
        let result = sqlx::query_as!(
            Holding,
            r#"WITH position AS (
                   INSERT INTO holdings (user_id, asset_id, quantity)
                   VALUES ($1, $2, $3)
                   ON CONFLICT (user_id, asset_id)
                   DO UPDATE SET quantity = holdings.quantity + EXCLUDED.quantity
                   RETURNING asset_id, quantity
               )
               SELECT p.asset_id AS "asset_id!",
                      a.name AS "name!",
                      a.unit_value AS "unit_value!",
                      p.quantity AS "quantity!"
               FROM position p
               JOIN assets a ON a.id = p.asset_id;"#,
            user_id,
            asset_id,
            quantity
        )
        .fetch_one(&self.db)
        .await;

        match result {
            Ok(holding) => Ok(holding),
            // A foreign key barra a compra de um ativo que não existe no catálogo.
            Err(sqlx::Error::Database(db_err)) if db_err.is_foreign_key_violation() => {
                Err(AppError::AssetDoesNotExist)
            }
            Err(err) => Err(AppError::Database(err)),
        }
    }

    /// Remove `quantity` do ativo da carteira.
    ///
    /// Se a venda zerar a posição, a linha é apagada em vez de ficar com
    /// quantidade zero: o `CHECK (quantity > 0)` da tabela não permitiria.
    /// Roda dentro de uma transação com `FOR UPDATE` para que duas vendas
    /// concorrentes não consigam vender mais do que a pessoa tem.
    pub async fn sell_asset(
        &self,
        user_id: i64,
        asset_id: i64,
        quantity: f64,
    ) -> Result<Option<Holding>, AppError> {
        let mut tx = self.db.begin().await?;

        let current = sqlx::query_scalar!(
            "SELECT quantity
             FROM holdings
             WHERE user_id = $1 AND asset_id = $2
             FOR UPDATE;",
            user_id,
            asset_id
        )
        .fetch_optional(&mut *tx)
        .await?;

        let Some(current) = current else {
            return Err(AppError::HoldingDoesNotExist);
        };

        if quantity > current {
            return Err(AppError::InsufficientQuantity {
                available: current,
                requested: quantity,
            });
        }

        let remaining = current - quantity;

        // Abaixo dessa margem a posição foi zerada; o resto é ruído de ponto
        // flutuante e não uma sobra real de ativo.
        const DUST: f64 = 1e-9;

        let holding = if remaining <= DUST {
            sqlx::query!(
                "DELETE FROM holdings
                 WHERE user_id = $1 AND asset_id = $2;",
                user_id,
                asset_id
            )
            .execute(&mut *tx)
            .await?;

            None
        } else {
            let holding = sqlx::query_as!(
                Holding,
                r#"WITH position AS (
                       UPDATE holdings
                       SET quantity = $3
                       WHERE user_id = $1 AND asset_id = $2
                       RETURNING asset_id, quantity
                   )
                   SELECT p.asset_id AS "asset_id!",
                          a.name AS "name!",
                          a.unit_value AS "unit_value!",
                          p.quantity AS "quantity!"
                   FROM position p
                   JOIN assets a ON a.id = p.asset_id;"#,
                user_id,
                asset_id,
                remaining
            )
            .fetch_one(&mut *tx)
            .await?;

            Some(holding)
        };

        tx.commit().await?;

        Ok(holding)
    }
}

impl FromRequestParts<AppState> for Repository {
    type Rejection = Infallible;

    async fn from_request_parts(
        _parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(Self {
            db: state.db.clone(),
        })
    }
}

#[cfg(test)]
impl From<PgPool> for Repository {
    fn from(db: PgPool) -> Self {
        Self { db }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ID da pessoa criada pela fixture `wallet`.
    const INVESTOR: i64 = 1;
    const BITCOIN: i64 = 1;
    const ETHEREUM: i64 = 2;

    #[sqlx::test(fixtures("wallet"))]
    async fn new_wallet_starts_empty(db: PgPool) {
        let repository = Repository::from(db);

        let wallet = repository.get_wallet(INVESTOR).await.expect("consulta ok");

        assert!(wallet.is_empty());
        assert_eq!(wallet.total_value(), 0.0);
    }

    #[sqlx::test(fixtures("wallet"))]
    async fn buying_creates_a_position(db: PgPool) {
        let repository = Repository::from(db);

        let holding = repository
            .buy_asset(INVESTOR, BITCOIN, 0.5)
            .await
            .expect("compra ok");

        assert_eq!(holding.name, "Bitcoin");
        assert_eq!(holding.quantity, 0.5);
        assert_eq!(holding.total_value(), 175_000.0);
    }

    #[sqlx::test(fixtures("wallet"))]
    async fn buying_the_same_asset_twice_adds_up(db: PgPool) {
        let repository = Repository::from(db);

        repository
            .buy_asset(INVESTOR, BITCOIN, 0.5)
            .await
            .expect("primeira compra");
        let holding = repository
            .buy_asset(INVESTOR, BITCOIN, 0.25)
            .await
            .expect("segunda compra");

        assert_eq!(holding.quantity, 0.75);

        // E continua sendo uma única linha na carteira, não duas.
        let wallet = repository.get_wallet(INVESTOR).await.expect("consulta ok");
        assert_eq!(wallet.len(), 1);
    }

    #[sqlx::test(fixtures("wallet"))]
    async fn buying_an_unknown_asset_is_rejected(db: PgPool) {
        let repository = Repository::from(db);

        let err = repository
            .buy_asset(INVESTOR, 999, 1.0)
            .await
            .expect_err("ativo inexistente deve falhar");

        assert!(matches!(err, AppError::AssetDoesNotExist));
    }

    #[sqlx::test(fixtures("wallet"))]
    async fn wallet_totals_every_position(db: PgPool) {
        let repository = Repository::from(db);

        repository
            .buy_asset(INVESTOR, BITCOIN, 0.5)
            .await
            .expect("compra btc");
        repository
            .buy_asset(INVESTOR, ETHEREUM, 3.0)
            .await
            .expect("compra eth");

        let wallet = repository.get_wallet(INVESTOR).await.expect("consulta ok");

        assert_eq!(wallet.len(), 2);
        // 0,5 x 350.000 + 3 x 12.400
        assert_eq!(wallet.total_value(), 212_200.0);
        // Ordenado por nome: Bitcoin antes de Ethereum.
        assert_eq!(wallet.holdings[0].name, "Bitcoin");
    }

    #[sqlx::test(fixtures("wallet"))]
    async fn selling_part_of_a_position_keeps_the_rest(db: PgPool) {
        let repository = Repository::from(db);

        repository
            .buy_asset(INVESTOR, BITCOIN, 1.0)
            .await
            .expect("compra");

        let remaining = repository
            .sell_asset(INVESTOR, BITCOIN, 0.4)
            .await
            .expect("venda ok")
            .expect("a posição deve continuar existindo");

        assert!((remaining.quantity - 0.6).abs() < 1e-9);
    }

    #[sqlx::test(fixtures("wallet"))]
    async fn selling_everything_closes_the_position(db: PgPool) {
        let repository = Repository::from(db);

        repository
            .buy_asset(INVESTOR, BITCOIN, 2.0)
            .await
            .expect("compra");

        let remaining = repository
            .sell_asset(INVESTOR, BITCOIN, 2.0)
            .await
            .expect("venda ok");

        assert!(remaining.is_none(), "a posição deveria ter sido encerrada");

        let wallet = repository.get_wallet(INVESTOR).await.expect("consulta ok");
        assert!(wallet.is_empty());
    }

    #[sqlx::test(fixtures("wallet"))]
    async fn cannot_sell_more_than_owned(db: PgPool) {
        let repository = Repository::from(db);

        repository
            .buy_asset(INVESTOR, BITCOIN, 1.0)
            .await
            .expect("compra");

        let err = repository
            .sell_asset(INVESTOR, BITCOIN, 1.5)
            .await
            .expect_err("vender além do saldo deve falhar");

        assert!(matches!(
            err,
            AppError::InsufficientQuantity {
                available,
                requested
            } if available == 1.0 && requested == 1.5
        ));

        // E a posição original continua intacta.
        let wallet = repository.get_wallet(INVESTOR).await.expect("consulta ok");
        assert_eq!(wallet.holdings[0].quantity, 1.0);
    }

    #[sqlx::test(fixtures("wallet"))]
    async fn cannot_sell_an_asset_never_bought(db: PgPool) {
        let repository = Repository::from(db);

        let err = repository
            .sell_asset(INVESTOR, ETHEREUM, 1.0)
            .await
            .expect_err("vender o que não se tem deve falhar");

        assert!(matches!(err, AppError::HoldingDoesNotExist));
    }

    #[sqlx::test(fixtures("wallet"))]
    async fn wallets_are_isolated_between_users(db: PgPool) {
        let repository = Repository::from(db);

        repository
            .buy_asset(INVESTOR, BITCOIN, 1.0)
            .await
            .expect("compra");

        let other = repository
            .add_user("outra-pessoa", "hash-sem-valor-em-testes")
            .await
            .expect("cadastro ok");

        let other_wallet = repository.get_wallet(other.id).await.expect("consulta ok");

        assert!(
            other_wallet.is_empty(),
            "a carteira de uma pessoa não pode vazar para outra"
        );
    }
}
