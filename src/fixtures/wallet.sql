-- Uma pessoa e dois ativos no catálogo, para os testes da carteira.
INSERT INTO users (id, username, password_hash)
VALUES (1, 'investidor', 'hash-sem-valor-em-testes');

INSERT INTO assets (id, name, unit_value)
VALUES (1, 'Bitcoin', 350000.0),
       (2, 'Ethereum', 12400.0);

-- Os IDs acima foram inseridos à mão, então as sequências precisam avançar
-- para que um INSERT seguinte não tente reusar um ID já ocupado.
SELECT setval('users_id_seq', (SELECT MAX(id) FROM users));
SELECT setval('assets_id_seq', (SELECT MAX(id) FROM assets));
