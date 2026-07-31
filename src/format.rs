//! Formatação de números para leitura humana, no padrão brasileiro.
//!
//! Fica em Rust, e não no template, porque é lógica com casos de borda que
//! merecem teste — e porque o Askama não tem um filtro de moeda embutido.

/// Formata um valor monetário: `212200.0` vira `212.200,00`.
pub fn brl(value: f64) -> String {
    let negative = value < 0.0;
    let cents = (value.abs() * 100.0).round() as u128;

    let reais = cents / 100;
    let remainder = cents % 100;

    let mut formatted = group_thousands(reais);
    formatted.push(',');
    formatted.push_str(&format!("{remainder:02}"));

    if negative {
        format!("-{formatted}")
    } else {
        formatted
    }
}

/// Formata uma quantidade de ativo: `0.5` vira `0,5` e `3.0` vira `3`.
///
/// Ativos como cripto são fracionários, então mantém até 8 casas decimais —
/// mas sem os zeros à direita, que só poluem a tabela.
pub fn quantity(value: f64) -> String {
    let text = format!("{value:.8}");
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');

    let (integer_part, decimal_part) = match trimmed.split_once('.') {
        Some((integer, decimal)) => (integer, Some(decimal)),
        None => (trimmed, None),
    };

    let integer: u128 = integer_part
        .trim_start_matches('-')
        .parse()
        .unwrap_or_default();

    let mut formatted = group_thousands(integer);

    if let Some(decimal) = decimal_part {
        formatted.push(',');
        formatted.push_str(decimal);
    }

    if value < 0.0 {
        format!("-{formatted}")
    } else {
        formatted
    }
}

/// Formata uma porcentagem com uma casa decimal: `75.0` vira `75,0`.
pub fn percent(value: f64) -> String {
    format!("{value:.1}").replace('.', ",")
}

/// Insere o ponto separador de milhar: `212200` vira `212.200`.
fn group_thousands(value: u128) -> String {
    let digits = value.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);

    for (index, digit) in digits.chars().enumerate() {
        // Um separador a cada três dígitos, contando a partir da direita.
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push('.');
        }
        grouped.push(digit);
    }

    grouped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_currency_with_thousand_separators() {
        assert_eq!(brl(212_200.0), "212.200,00");
        assert_eq!(brl(1_234_567.89), "1.234.567,89");
        assert_eq!(brl(999.0), "999,00");
        assert_eq!(brl(0.0), "0,00");
    }

    #[test]
    fn formats_currency_below_one_real() {
        assert_eq!(brl(0.5), "0,50");
        assert_eq!(brl(0.05), "0,05");
    }

    #[test]
    fn rounds_currency_to_cents() {
        assert_eq!(brl(10.005), "10,01");
        assert_eq!(brl(10.004), "10,00");
    }

    #[test]
    fn formats_negative_currency() {
        assert_eq!(brl(-1_500.25), "-1.500,25");
    }

    #[test]
    fn quantity_drops_trailing_zeros() {
        assert_eq!(quantity(3.0), "3");
        assert_eq!(quantity(0.5), "0,5");
        assert_eq!(quantity(1.25), "1,25");
    }

    #[test]
    fn quantity_keeps_small_fractions() {
        assert_eq!(quantity(0.000_005), "0,000005");
    }

    #[test]
    fn quantity_groups_large_amounts() {
        assert_eq!(quantity(1_500_000.0), "1.500.000");
    }

    #[test]
    fn formats_percent_with_one_decimal() {
        assert_eq!(percent(75.0), "75,0");
        assert_eq!(percent(33.333), "33,3");
    }
}
