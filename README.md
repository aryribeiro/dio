# Carteira de Investimentos — Rust Fullstack

Aplicação web para acompanhar uma carteira de investimentos: você entra com sua
conta, registra compras e vendas de ativos e vê quanto sua carteira vale hoje.

Projeto do desafio **Rust Fullstack — Carteira de Investimentos** da
[Digital Innovation One](https://www.dio.me), construído a partir do
[repositório base](https://github.com/digitalinnovationone/rust-fullstack-carteira-investimentos)
e evoluído com a melhoria descrita abaixo.

<p>
  <img alt="Rust" src="https://img.shields.io/badge/Rust-2024-000000?logo=rust&logoColor=white">
  <img alt="Axum" src="https://img.shields.io/badge/Axum-0.8-6633cc">
  <img alt="PostgreSQL" src="https://img.shields.io/badge/PostgreSQL-18-4169E1?logo=postgresql&logoColor=white">
  <img alt="Testes" src="https://img.shields.io/badge/testes-35%20passando-2ea44f">
  <img alt="Clippy" src="https://img.shields.io/badge/clippy-sem%20avisos-2ea44f">
</p>

---

## Sumário

- [O que o projeto faz](#o-que-o-projeto-faz)
- [Qual melhoria foi implementada](#qual-melhoria-foi-implementada)
- [Estatísticas do trabalho](#estatísticas-do-trabalho)
- [Tecnologias usadas](#tecnologias-usadas)
- [Como executar](#como-executar)
- [Como testar](#como-testar)
- [Rotas](#rotas)
- [Estrutura do projeto](#estrutura-do-projeto)
- [Decisões de projeto](#decisões-de-projeto)
- [O que eu aprendi](#o-que-eu-aprendi)
- [Limitações conhecidas](#limitações-conhecidas)

---

## O que o projeto faz

- **Autenticação** — cadastro e login com senha protegida por hash Argon2. A
  sessão é mantida por um token JWT guardado em cookie.
- **Catálogo de ativos** — uma API REST cria, lista e atualiza os ativos
  disponíveis (nome e valor unitário). Alterar o catálogo exige credencial de
  administrador.
- **Carteira pessoal** — cada pessoa registra quanto possui de cada ativo. As
  carteiras são isoladas: ninguém vê a posição de ninguém.
- **Dashboard** — uma página mostra o valor total da carteira, cada posição com
  seu total e o quanto cada ativo representa do patrimônio.

---

## Qual melhoria foi implementada

O projeto base tinha uma tabela de ativos e uma tela de login, mas **não tinha
carteira**. Depois de entrar, a pessoa via exatamente isto:

```rust
// src/routes/frontend.rs, no projeto base
Some(user) => Ok(Html(format!("Hello, {}", user.username())).into_response()),
```

Uma string. Não havia como registrar o que se possui, nem saber quanto isso
vale. A melhoria fecha essa lacuna.

### 1. A carteira propriamente dita

Uma tabela nova, `holdings`, liga pessoa ↔ ativo ↔ quantidade, com chave
primária composta `(user_id, asset_id)`. Isso garante, **no próprio banco**, que
não existam duas linhas para o mesmo ativo na mesma carteira.

A compra usa `INSERT ... ON CONFLICT DO UPDATE`, então comprar um ativo que já
está na carteira **soma** à posição existente em uma única operação atômica —
duas compras simultâneas não se sobrescrevem.

A venda roda dentro de uma transação com `SELECT ... FOR UPDATE`, para que duas
vendas concorrentes não consigam vender mais do que a pessoa tem. Quando a venda
zera a posição, a linha é removida em vez de ficar com quantidade zero.

### 2. Dashboard

A rota `/` deixou de ser uma string e virou uma página que mostra:

- o **valor total da carteira**, somando quantidade × valor unitário de tudo;
- uma tabela de posições com quantidade, valor unitário, total e a
  **participação de cada ativo** no patrimônio;
- formulário de compra com os ativos do catálogo;
- venda direta em cada linha da tabela;
- um estado vazio explicativo para quem ainda não comprou nada.

Valores aparecem no formato brasileiro (`R$ 212.200,00`) e quantidades mantêm
até 8 casas decimais sem zeros à direita, para acomodar ativos fracionários.

### 3. Validações e mensagens de erro

- Quantidade precisa ser um número finito maior que zero — `NaN` e infinito são
  recusados antes de chegar ao banco.
- Vender mais do que se tem devolve uma mensagem que diz quanto você tem e
  quanto tentou vender, em vez de um erro de constraint do Postgres.
- Senha exige no mínimo 8 caracteres; nome de usuário aceita até 40 caracteres
  entre letras, números, ponto, hífen e underline.
- Erros em rotas de navegador viram **página HTML**; erros de API continuam
  JSON. Falhas internas (banco, template) mostram uma mensagem genérica — o
  detalhe vai para o log, não para a tela.

### 4. Segurança

- As chaves de assinatura de token e de administrador **saíram do código-fonte**
  e passaram a vir do ambiente, com validação de tamanho mínimo no boot. Antes
  eram constantes literais (`b"im-so-secret"`, `"im-the-admin"`) — qualquer
  pessoa com acesso ao repositório conseguiria forjar um token de sessão válido.
- O cookie de sessão ganhou `SameSite=Lax`, `Path=/` e expiração alinhada ao
  token. Antes o cookie era permanente enquanto o token durava 10 minutos, o que
  deslogava a pessoa em silêncio.
- Passou a existir **logout**.

### 5. Testes

A suíte saiu de 3 para 35 testes, entre eles:

- toda a mecânica de compra e venda contra um Postgres real — venda parcial,
  venda total, e os dois casos de erro;
- **isolamento entre carteiras**: a posição de uma pessoa não pode aparecer na
  carteira de outra;
- cálculo de total e participação, incluindo a carteira vazia (divisão por zero);
- formatação de moeda, quantidade e porcentagem, com arredondamento de centavos;
- validação de segredos curtos ou ausentes.

---

## Estatísticas do trabalho

Comparação entre o [repositório base da DIO](https://github.com/digitalinnovationone/rust-fullstack-carteira-investimentos)
e esta entrega. Todos os números são reproduzíveis com os comandos indicados.

| Métrica | Base DIO | Esta entrega | Variação |
| --- | ---: | ---: | ---: |
| Linhas de Rust (`src/`) | 556 | 1.684 | **+203%** |
| Arquivos `.rs` | 11 | 13 | +2 |
| Testes automatizados | 3 | **35** | +32 |
| Templates HTML | 1 | 4 | +3 |
| Linhas de HTML | 35 | 194 | +454% |
| Tabelas no banco | 2 | 3 | +1 |
| Arquivos de migração | 4 | 6 | +2 |
| Rotas de navegador | 2 | 5 | +3 |
| Endpoints de API | 3 | 3 | — |

<details>
<summary>Como reproduzir esses números</summary>

```bash
# Linhas de Rust
find src -name "*.rs" -exec cat {} \; | wc -l

# Testes (unitários + de integração com banco)
grep -rhoE "#\[(test|sqlx::test)" src --include="*.rs" | wc -l

# Rotas de navegador
grep -ohE '\.route\("[^"]+"' src/routes/frontend.rs | wc -l
```

</details>

### Detalhamento por arquivo

| Arquivo | Base | Entrega | Situação |
| --- | ---: | ---: | --- |
| `src/repository.rs` | 104 | 439 | reescrito — queries da carteira + 10 testes |
| `src/routes/frontend.rs` | 61 | 272 | reescrito — dashboard, compra, venda, logout |
| `src/error.rs` | 47 | 192 | reescrito — erros em HTML e JSON |
| `src/models.rs` | 14 | 157 | reescrito — `Holding`, `Wallet` e cálculos |
| `src/auth/user.rs` | 124 | 166 | validações + segredo do ambiente |
| `src/format.rs` | — | 129 | **novo** — moeda e quantidade em pt-BR |
| `src/config.rs` | — | 110 | **novo** — segredos lidos do ambiente |
| `src/app.rs` | 50 | 62 | migrações no boot |
| `src/auth/admin.rs` | 26 | 24 | segredo do ambiente |
| `src/main.rs` | 13 | 16 | novos módulos |
| `src/routes/api.rs` | 113 | 113 | **intocado** |
| `templates/dashboard.html` | — | 129 | **novo** |
| `templates/base.html` | — | 18 | **novo** — layout compartilhado |
| `templates/error.html` | — | 13 | **novo** |
| `templates/login.html` | 35 | 34 | passou a herdar do layout |

Também foram criados: a migração `holdings`, uma fixture de testes,
`.env.example`, `rust-toolchain.toml` e os 10 arquivos de metadados em `.sqlx/`.

### Verificação executada

A aplicação não foi apenas compilada — foi **executada e percorrida de ponta a
ponta** por HTTP antes da entrega:

| Cenário | Resultado esperado | Verificado |
| --- | --- | --- |
| `GET /` sem sessão | redireciona para `/login` | 303 → `/login` |
| Login criando conta | cookie `HttpOnly` com expiração | 303 + cookie |
| `POST /api/assets` sem credencial | recusado | 400 |
| Compra de 0,5 BTC + 3 ETH | R$ 212.200,00 | R$ 212.200,00 |
| Participação do Bitcoin | 82,5% | 82,5% |
| Vender mais do que possui | erro explicativo | 400 + mensagem |
| Vender ativo que não possui | não encontrado | 404 |
| Quantidade zero, negativa ou `NaN` | recusada | 400 |
| Venda total | posição encerrada | 303 + posição removida |
| Segunda conta vê carteira alheia | não | carteira vazia |
| Logout | sessão encerrada | 303 → `/login` |
| Build com o Postgres **desligado** | compila via `.sqlx/` | compilou |

Além disso: `cargo test` com 35 testes passando, `cargo clippy --all-targets`
sem nenhum aviso e `cargo fmt` aplicado.

---

## Tecnologias usadas

| Camada | Ferramenta |
| --- | --- |
| Linguagem | Rust (edição 2024) |
| Servidor web | [Axum](https://github.com/tokio-rs/axum) 0.8 |
| Runtime assíncrono | [Tokio](https://tokio.rs) |
| Banco de dados | PostgreSQL 18 via [SQLx](https://github.com/launchbadge/sqlx) 0.8 |
| Templates HTML | [Askama](https://github.com/askama-rs/askama) 0.15 |
| Senhas | [password-auth](https://crates.io/crates/password-auth) (Argon2) |
| Sessão | [jwt-simple](https://crates.io/crates/jwt-simple) (HS256) |
| Estilo | Tailwind CSS via CDN |
| Testes | `sqlx::test` + [insta](https://insta.rs) |
| Infra local | Docker Compose |

---

## Como executar

### Pré-requisitos

- [Rust](https://rustup.rs) 1.85 ou superior (a edição 2024 exige essa versão)
- [Docker](https://docs.docker.com/get-docker/) com Compose

> **No Windows**, o Rust precisa de um linker. Instale os *Visual Studio Build
> Tools* com o workload **Desenvolvimento para desktop com C++** e o Windows SDK:
> ```powershell
> winget install Microsoft.VisualStudio.2022.BuildTools --override `
>   "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
> ```

### Passo a passo

```bash
# 1. Configure o ambiente
cp .env.example .env
# edite o .env e troque JWT_SECRET e ADMIN_SECRET por valores próprios

# 2. Suba o banco
docker compose up -d

# 3. Rode a aplicação (as migrações são aplicadas sozinhas no boot)
cargo run
```

A aplicação sobe em <http://localhost:3000>.

> **Sobre o `SQLX_OFFLINE=true`:** as macros do SQLx conferem cada query contra o
> banco **durante a compilação**. Os metadados dessa conferência estão
> versionados em `.sqlx/`, então o projeto compila mesmo com o Postgres
> desligado. Se você alterar alguma query, rode `cargo sqlx prepare` com o banco
> no ar para regravar esses metadados.

### Primeiro acesso

1. Abra <http://localhost:3000>; você será levado para a tela de login.
2. Digite um usuário e uma senha de pelo menos 8 caracteres. Se a conta não
   existir, ela é criada na hora.
3. A carteira aparece vazia — é esperado, o catálogo de ativos ainda está sem
   nada.
4. Cadastre alguns ativos pela API (use o `ADMIN_SECRET` do seu `.env`):

```bash
curl -X POST http://localhost:3000/api/assets \
  -H "Authorization: SEU_ADMIN_SECRET" \
  -H "Content-Type: application/json" \
  -d '{"name": "Bitcoin", "unit_value": 350000.0}'

curl -X POST http://localhost:3000/api/assets \
  -H "Authorization: SEU_ADMIN_SECRET" \
  -H "Content-Type: application/json" \
  -d '{"name": "Ethereum", "unit_value": 12400.0}'
```

5. Recarregue o dashboard: os ativos agora aparecem no formulário de compra.
   Compre `0,5` de Bitcoin e `3` de Ethereum — o total deve marcar
   **R$ 212.200,00**, com Bitcoin representando **82,5%** do patrimônio.

---

## Como testar

Os testes sobem bancos temporários isolados, então o Docker precisa estar no ar:

```bash
docker compose up -d
cargo test
```

Para conferir a saúde geral do código:

```bash
cargo clippy --all-targets
cargo fmt --check
```

---

## Rotas

### Navegador

| Método | Rota | O que faz |
| --- | --- | --- |
| `GET` | `/` | Dashboard da carteira (redireciona para o login se não autenticado) |
| `GET` | `/login` | Formulário de entrada |
| `POST` | `/login` | Autentica ou cria a conta |
| `POST` | `/logout` | Encerra a sessão |
| `POST` | `/carteira/comprar` | Registra uma compra |
| `POST` | `/carteira/vender` | Registra uma venda |

### API

| Método | Rota | Autenticação | O que faz |
| --- | --- | --- | --- |
| `GET` | `/api/assets` | — | Lista o catálogo de ativos |
| `POST` | `/api/assets` | `Authorization: <ADMIN_SECRET>` | Cadastra um ativo |
| `PATCH` | `/api/assets` | `Authorization: <ADMIN_SECRET>` | Atualiza nome ou valor |

---

## Estrutura do projeto

```
src/
├── main.rs           ponto de entrada
├── app.rs            boot: logs, configuração, migrações, rotas
├── config.rs         segredos lidos do ambiente
├── error.rs          erros da aplicação em JSON (API) e HTML (navegador)
├── format.rs         moeda e quantidade no padrão brasileiro
├── models.rs         Asset, Holding, Wallet e os cálculos da carteira
├── repository.rs     todo o acesso ao banco
├── auth/
│   ├── admin.rs      credencial de administrador da API
│   └── user.rs       cadastro, login e token de sessão
└── routes/
    ├── api.rs        endpoints JSON
    └── frontend.rs   páginas e formulários

templates/            base, login, dashboard e página de erro
migrations/           evolução do banco (assets, users, holdings)
.sqlx/                metadados das queries, para compilar sem banco
```

### Modelo de dados

```
users                 assets                holdings
─────                 ──────                ────────
id         BIGSERIAL  id       BIGSERIAL    user_id   ─┐ FK → users.id
username   UNIQUE     name     UNIQUE       asset_id  ─┤ FK → assets.id
password_hash         unit_value            quantity   │ CHECK > 0
                                            PK (user_id, asset_id)
```

---

## Decisões de projeto

**Por que a posição, e não o extrato.** `holdings` guarda *quanto se tem agora*,
não a sequência de operações. É o suficiente para responder "quanto vale minha
carteira", que é a pergunta do desafio, e mantém a leitura do dashboard em uma
única consulta. O custo está registrado em [limitações](#limitações-conhecidas).

**Por que a venda usa transação, e a compra não.** A compra é uma única
instrução (`INSERT ... ON CONFLICT`), e o Postgres já a executa atomicamente. A
venda precisa *ler para decidir* — se sobra posição ou se ela é encerrada — e
essa leitura precisa de `FOR UPDATE` para não competir com outra venda.

**Por que os erros têm dois formatos.** O mesmo `AppError` serve à API e ao
navegador; um invólucro (`HtmlError`) escolhe entre JSON e página HTML. Sem
isso, um erro de venda devolveria JSON cru na cara de quem clicou num botão.

**Por que as conversões de erro são escritas uma a uma.** Um `impl` genérico
sobre `Into<AppError>` colidiria com a conversão reflexiva da biblioteca padrão.
O compilador rejeita — está anotado no código para não ser "simplificado" depois.

**Por que `jwt-simple` em `pure-rust`.** A configuração padrão traz o BoringSSL,
que exige CMake e um compilador C++. Com a feature `pure-rust`, o projeto compila
com o toolchain do Rust e mais nada.

---

## O que eu aprendi

**Extractors do Axum são o ponto de organização do projeto.** `Repository`,
`User` e `Admin` implementam `FromRequestParts`, então um handler declara o que
precisa na assinatura e o Axum resolve. Autorização deixa de ser um `if` no
começo da função e vira um tipo: se `Admin` está na assinatura, a rota é
protegida — não tem como esquecer.

**`Option<User>` como extractor separa "não logado" de "erro".** É o que permite
o dashboard redirecionar para o login em vez de devolver 401.

**As macros do SQLx conferem SQL em tempo de compilação.** Um nome de coluna
errado não passa do `cargo build`. Em troca, elas precisam de um banco acessível
durante a compilação — ou dos metadados versionados em `.sqlx/`. Descobri isso
da pior forma: derrubei o banco para testar as migrações no boot e o projeto
parou de compilar, então nem dava para rodar a aplicação que criaria as tabelas.

**Restrições no banco valem mais que validação na aplicação.** O
`CHECK (quantity > 0)` e a chave primária composta impedem estados inválidos
mesmo que a aplicação tenha um bug. A validação em Rust existe para produzir uma
*mensagem boa*, não para ser a única linha de defesa.

**Ponto flutuante e dinheiro se dão mal.** Vender exatamente tudo que se tem pode
deixar um resíduo como `2,7e-17` em vez de zero. Por isso a venda trata qualquer
sobra abaixo de `1e-9` como posição encerrada.

**Segredo em constante de código é segredo vazado.** `b"im-so-secret"` no fonte
significa que quem lê o repositório assina tokens válidos para qualquer usuário.
Mover para o ambiente foi de longe a correção de maior impacto por linha escrita.

---

## Limitações conhecidas

- **Valores são `f64`.** O ideal para dinheiro seria `NUMERIC` no Postgres com um
  tipo decimal em Rust. Manteve-se `f64` por consistência com o projeto base; a
  migração é a próxima evolução natural.
- **Não há histórico de operações.** A carteira guarda a posição atual, não a
  sequência de compras e vendas — então não há preço médio nem cálculo de lucro.
- **Os preços são cadastrados à mão.** Não existe integração com cotação de
  mercado.
- **O login cadastra quem ainda não tem conta.** É o fluxo do projeto base,
  mantido para não exigir dois formulários; em um sistema real o cadastro seria
  uma tela separada.
- **O catálogo de ativos só é gerenciado por API.** Não há tela de administração;
  criar ou atualizar ativos exige `curl` ou equivalente.

---

## Licença

Projeto educacional, desenvolvido para o desafio da Digital Innovation One.
