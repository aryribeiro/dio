use std::sync::OnceLock;

use color_eyre::eyre::{Context, eyre};
use jwt_simple::prelude::HS256Key;

/// Segredos da aplicação, lidos do ambiente uma única vez no boot.
///
/// Antes eles eram constantes no código-fonte, o que significa que qualquer
/// pessoa com acesso ao repositório conseguia forjar um token de sessão.
pub struct Secrets {
    jwt_key: HS256Key,
    admin_secret: String,
}

impl Secrets {
    pub const fn jwt_key(&self) -> &HS256Key {
        &self.jwt_key
    }

    pub fn admin_secret(&self) -> &str {
        &self.admin_secret
    }
}

static SECRETS: OnceLock<Secrets> = OnceLock::new();

/// Menor tamanho aceitável para um segredo. Uma chave HS256 curta é vulnerável
/// a força bruta offline: quem tiver um token consegue descobrir a chave e
/// emitir tokens válidos para qualquer usuário.
const MIN_SECRET_LEN: usize = 32;

fn read_secret(name: &str) -> color_eyre::Result<String> {
    let value = std::env::var(name)
        .wrap_err_with(|| format!("a variável de ambiente {name} não está definida"))?;

    if value.len() < MIN_SECRET_LEN {
        return Err(eyre!(
            "{name} tem {} caracteres, mas precisa de pelo menos {MIN_SECRET_LEN}. \
             Gere um valor seguro com `openssl rand -base64 48`.",
            value.len()
        ));
    }

    Ok(value)
}

/// Carrega os segredos do ambiente. Deve ser chamada uma vez, no boot.
pub fn init() -> color_eyre::Result<()> {
    let jwt_secret = read_secret("JWT_SECRET")?;
    let admin_secret = read_secret("ADMIN_SECRET")?;

    let secrets = Secrets {
        jwt_key: HS256Key::from_bytes(jwt_secret.as_bytes()),
        admin_secret,
    };

    // Um segundo `init` não deveria acontecer; se acontecer, o primeiro valor
    // continua valendo e nada explode.
    let _ = SECRETS.set(secrets);

    Ok(())
}

/// Acesso aos segredos carregados.
///
/// Nos testes não existe boot, então usa um valor fixo — os testes nunca tocam
/// em tokens reais.
pub fn secrets() -> &'static Secrets {
    #[cfg(test)]
    {
        SECRETS.get_or_init(|| Secrets {
            jwt_key: HS256Key::from_bytes(b"chave-de-teste-sem-valor-em-producao"),
            admin_secret: "segredo-de-teste-sem-valor-em-producao".to_string(),
        })
    }

    #[cfg(not(test))]
    {
        SECRETS
            .get()
            .expect("config::init() precisa rodar antes de qualquer requisição")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_short_secrets() {
        unsafe { std::env::set_var("TEST_SHORT_SECRET", "curto") };

        let err = read_secret("TEST_SHORT_SECRET").expect_err("segredo curto deve falhar");
        assert!(err.to_string().contains("pelo menos"));
    }

    #[test]
    fn accepts_long_secrets() {
        let long = "a".repeat(MIN_SECRET_LEN);
        unsafe { std::env::set_var("TEST_LONG_SECRET", &long) };

        assert_eq!(read_secret("TEST_LONG_SECRET").expect("deve aceitar"), long);
    }

    #[test]
    fn reports_missing_variable_by_name() {
        let err = read_secret("TEST_ABSENT_SECRET").expect_err("variável ausente deve falhar");
        assert!(err.to_string().contains("TEST_ABSENT_SECRET"));
    }
}
